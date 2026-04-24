# Phase 6a §7 commitment gate — attempt status

> **Status (2026-04-24):** **Landed.** Root cause identified as an LLVM optimization cliff (session 3); fix was splitting `resolve_goap_plans` into two functions and wiring the prologue gate into the smaller one (session 4). See §"Mystery resolved: LLVM optimization cliff" and §"Disposition — split `resolve_goap_plans`, then re-wire the prologue gate" below.

This doc preserves the full investigation trail (bisection methodology, falsified hypotheses, LLVM cliff root cause). See also:

- `docs/systems/ai-substrate-refactor.md` §7 — spec (authoritative).
- `docs/systems/refactor-plan.md` Phase 6a — plan-of-record.
- `docs/open-work.md` #5 "§7 commitment strategies" — live status; update this doc's pointer when the next attempt starts.

## What landed in the working copy (session 2)

Changes relative to main (`2d3ba96a`), 19 files:

| File | Purpose |
|---|---|
| `src/ai/commitment.rs` (new, ~850 lines) | `BeliefProxies` struct, `should_drop_intention` pure-function gate, §7.3 `strategy_for_disposition` 12-row table, `proxies_for_plan` belief-proxy recipe, `record_drop` telemetry helper. 25 unit tests including the `trips_done > 0` guard. |
| `src/ai/mod.rs` | `pub mod commitment;`. |
| `src/ai/dses/{eat,explore,fight,fight_target,flee,groom_other,groom_self,mentor,mentor_target,patrol,practice_magic,sleep,socialize}.rs` | `default_strategy()` impls per §7.3 table. Purely declarative — `default_strategy` is never called today, it's metadata for future wiring. |
| `src/resources/system_activation.rs` | `Feature::CommitmentDropTriggered` variant (Neutral, not expected-to-fire-per-soak) + classification + tests updated. |
| `src/plugins/simulation.rs` + `src/main.rs::build_schedule` | Comments documenting why `reconsider_held_intentions` is *not* registered. |
| `CLAUDE.md` | "§7.2 commitment gate — mental model" subsection under "AI Substrate Refactor". Encodes the belief-proxy semantics + the lifted-condition lesson from the first-attempt regression. |

Total: 958 insertions, 31 deletions. `just check` clean, `just test` passes (1119 tests, 25 in `ai::commitment::tests`).

**What is NOT wired:**

- No `reconsider_held_intentions` system registered in the schedule (first attempt's shape).
- No gate logic inside `resolve_goap_plans` (second attempt's shape).
- `default_strategy()` is never consumed.
- `Feature::CommitmentDropTriggered` is never recorded (never fires).

The gate is effectively **inert code + tests**. Soak behavior is byte-identical to main at the behavioral level (identical to H2 measurements).

## Session 2 timeline

### Session 1 attempt (morning, 2026-04-23 PM) — separate `reconsider_held_intentions` system

Shape: stand-alone Bevy system `.after(check_anxiety_interrupts).before(evaluate_and_plan)`. Signature carried `Query<(Entity, &GoapPlan, &Needs), Without<Dead>>` + `ResMut<SystemActivation>` + `Commands` + `Res<SimConstants>`.

**Failed seed-42 soak:** Starvation 0→8, wards_placed 200→2, grooming 129→0. 11 never-fired expected positives. `CommitmentDropTriggered` fired only 8× colony-wide across 54k ticks — the gate's *logic* was barely executing, yet the colony collapsed. Draft preserved on jj bookmark `session-c-draft` at `aa996a1c`.

### Session 2 (evening, 2026-04-23) — this document

#### H1 bisection — "is the Resting proxy recipe over-eager?"

Theory: `achievement_believed` for `DispositionKind::Resting` lifted `goap.rs:1672`'s three-need threshold check out of its `trips_done += 1` block and fired at ambient baseline. Diagnostic: `DispositionKind::Resting => false` unconditionally.

**Result: H1 falsified.** Seed-42 soak still failed with same shape (Starvation=8, 11 never-fired, colony wipe). Force-false didn't rescue the colony. Trace preserved at `logs/h1-diagnostic-42/`.

#### H2 bisection — "is the schedule presence the issue?"

Theory: the gate's `ResMut<SystemActivation>` adds a 20th writer on that resource, forcing serialization reshuffle that delays `check_anxiety_interrupts` / `evaluate_and_plan` relative to other systems. Diagnostic: un-register the gate, keep the module code.

**Result: H2 cleared.** Starvation=0, grooming=189, wards_placed=233. Colony survived. Preserved at `logs/h2-unregistered-42/` — this is the **known-good reference state** and matches the current working copy behaviorally.

#### Merge-inline attempt — "put the gate into `resolve_goap_plans`'s per-cat loop"

Rationale: merge the gate into the system that already owns `&mut GoapPlan`, `Commands`, `plans_to_remove: Vec<Entity>`, and `narr.activation`. No new scheduler edges; no new `ResMut<SystemActivation>` writer. Insertion site: `goap.rs:1652`, top of the per-cat loop body, before `if plan.is_exhausted()`.

Body (7 lines): look up strategy, compute proxies, if-drop push to `plans_to_remove` + `record_drop`, fall through (first variant had `continue`, second variant removed it after suspecting determinism break).

**Result: both variants failed.** Second variant (no `continue`): Starvation=8, grooming=0, wards_placed=0, 13 never-fired. Colony wipe over 22,784 ticks. Calcifer: 12 decisions, last at tick 1,201,721 with `goap_plan: []`, then 10k+ ticks stuck before starving. `CommitmentDropTriggered` fired 7× total colony-wide.

Parallel five-agent `/diagnose-run` surfaced:
- Cat hunting collapsed to zero after window 0 (3 kills total).
- `FoodEaten=3` across 22 sim-days for 8 cats (~2.8% of demand).
- `MaterialsDelivered=0` for 5 completed constructions.
- All continuity tallies zero (grooming, play, mentoring, burial, courtship, mythic-texture).
- User's manual run (interactive sim) observed: **"all of the cats explore and just all run northward."**

#### Adjacent find: `find_nearest_tile` north-bias

`src/systems/goap.rs:4258` iterates `dy = -radius..=radius` outer, `dx = -radius..=radius` inner, and keeps tiles via `is_none_or(|(_, d)| dist < d)` (strict less-than). For `PlannerZone::Wilds` (radius=20, predicate `t.is_passable()`), the first `dist=1` candidate found is the tile directly north (`dy=-1, dx=0`), and it's never beaten by later `dist=1` tiles (N→W→E→S iteration order, ties don't flip `best`). Every cat picking a Wilds target walks north.

This bias is **pre-existing** on main — not caused by the commitment gate — but manifests mildly there because cats have varied DSE picks that bleed off the Explore/Wander loop. On merge runs, cats get stuck in Explore loops and the bias becomes fatal colony-wide.

**Handoff:** user ("wnmitch") is fixing this in a separate session. Don't block Phase 6a on it.

#### Probe 3 — empty-body gate test

Diagnostic: replace the 7-line gate body with four literal field reads:

```rust
let _probe_kind = plan.kind;
let _probe_trips = plan.trips_done;
let _probe_hunger = needs.hunger;
let _probe_d = d.resting_complete_hunger;
```

No helper calls. No pushes. Purely shaped-like-the-gate-was code.

**Result: probe 3 cleared.** At 393s wall (43% of a full soak), Calcifer had made **1,082 decisions** vs. 12 in the failed merge. Trace file 154 MB vs 15–60 MB. 0-step plan recovery observed in 1 tick (ticks 1,308,120 → 1,308,122, three decisions in succession). Extrapolated to 900s: ~2,400 decisions, matching H2. Killed by user; partial logs likely overwritten by next probe.

**Conclusion: the gate's code shape is harmless; the regression lives in one of the four helper calls.**

## Mystery resolved: LLVM optimization cliff (H1 confirmed)

**Session 3 (2026-04-23 late evening)** confirmed the root cause via a
controlled debug-vs-release test. The exact prologue gate code from the
session 2 merge-inline attempt was re-inserted at line ~1825 of
`resolve_goap_plans` (after `is_exhausted()` block, before step
execution):

```rust
let strategy = strategy_for_disposition(plan.kind);
let proxies = proxies_for_plan(&plan, &needs, &ec.constants.disposition);
if should_drop_intention(strategy, proxies) {
    record_drop(narr.activation.as_deref_mut(), strategy, DropBranch::Achieved);
    plans_to_remove.push(cat_entity);
    continue;
}
```

**Results (seed 42, `--duration 120`):**

| Build mode | Starvation deaths | Footer written | Colony survived |
|---|---|---|---|
| Debug (`cargo run`) | 0 | ✓ | ✓ |
| Release (`cargo run --release`) | 7 | ✓ | ✗ |

Same code, same seed, same duration. The regression is **release-mode-only**.

### Why this happens

`resolve_goap_plans` is ~4,500 lines — one of the largest functions in the
codebase. LLVM has cost-based thresholds for inlining, register allocation,
and loop optimization. Adding four cross-module function calls to the hot
inner loop (executed every cat × every tick) pushes the function past an
optimization cliff. In release mode, LLVM either:

- Stops inlining critical step-resolver helpers deeper in the function body
- Spills registers that were previously kept live across the loop iteration
- Changes instruction scheduling in a way that affects Bevy's change-detection
  tick comparisons or plan-state reads

The empty-body probe (field reads only) passed because field reads don't
affect LLVM's complexity analysis — they're trivial inline operations, not
function call boundaries.

### Supporting evidence from session 3 instrumented soak

A parallel session wired `strategy_for_disposition` + `record_drop` at
the two EXISTING decision points (`disposition_complete` and
`max_replans_exceeded`). These calls execute **6,655 times** inside
conditional branches (~0.6% of loop iterations). Colony survived. The
same functions that kill the colony when called on 100% of iterations
work fine when called on 0.6%.

### Fix paths

1. **`#[inline(always)]`** on the commitment helpers — forces LLVM to
   inline them, removing the function-call boundary that triggers the
   optimization change. Fragile: any future change to the helpers or the
   function could re-trigger the cliff.
2. **Split `resolve_goap_plans`** into smaller functions so no single
   function crosses the optimization threshold. Correct long-term fix
   but high-effort.
3. **Avoid the hot-path prologue entirely** — extend the existing
   cold-branch decision points with desire-drift semantics instead of
   adding a per-iteration gate. The session 3 analysis showed the
   prologue gate's novel behavioral surface is narrow (mainly
   Socializing mid-trip desire-drift); everything else is already
   handled by existing code paths.

### Disposition — split `resolve_goap_plans`, then re-wire the prologue gate

The correct fix is **splitting `resolve_goap_plans` into smaller
functions** so no single function body crosses LLVM's optimization
cliff. This is the right long-term investment: the function is already
~4,500 lines and will only grow as new step resolvers land. The split
also unblocks the prologue gate — once the per-cat loop body is a
reasonable size, adding four cross-module function calls to the hot
path should be within LLVM's optimization budget.

**Split landed.** The step-resolution dispatch (`match action_kind { ... }`)
was extracted into `dispatch_step_action` (marked `#[inline(never)]`).
Result: `resolve_goap_plans` is ~764 lines, `dispatch_step_action` is
~1,269 lines — both well under the cliff. Immutable pre-loop snapshots
are bundled in a `StepSnapshots` struct; mutable accumulators (mentor
effects, grooming restorations, kitten feedings) in `StepAccumulators`.
Debug-vs-release test (seed 42, `--duration 120`) confirmed identical
behavior in both modes.

**Gate landed (session 4, 2026-04-24).** The gate is wired into the
prologue of `resolve_goap_plans` (the 764-line function, after the
split). It evaluates `should_drop_intention` per cat per tick: if
the strategy says drop, the plan is removed and the cat re-enters
`evaluate_and_plan` next tick.

The full gate shape (with `ec_is_focal` / `record_commitment_decision`
focal-trace capture on both the drop and retained paths) triggered the
LLVM regression even with `#[inline(always)]` on the commitment
helpers. The compiled-but-never-taken focal-trace code path was enough
to push the optimization budget. The shipped shape omits focal-trace
capture from the per-tick gate; `record_drop` telemetry (which fires
only on the rare drop path) is retained. Focal-trace coverage for
commitment decisions remains at the existing `disposition_complete` and
`max_replans_exceeded` sites, which run on the cold path (~0.6% of
loop iterations).

The commitment helpers (`strategy_for_disposition`, `proxies_for_plan`,
`should_drop_intention`, `record_drop`) remain as the source of truth
for §7.1–§7.3 semantics. The session 3 instrumented telemetry at
existing decision points (`disposition_complete` and
`max_replans_exceeded` branches) stays in place — it's valuable
regardless of whether the prologue gate lands.

## Previous bisection steps (sessions 1–2, superseded by H1 confirmation)

The original four-candidate bisection plan is no longer needed. For the
historical record, the candidates were:

1. `strategy_for_disposition(plan.kind)` — pure match, no refs
2. `proxies_for_plan(&plan, &needs, d)` — shared refs (was hypothesis-of-leading)
3. `should_drop_intention(strategy, proxies)` — pure match on Copy types
4. `record_drop(narr.activation.as_deref_mut())` — fires only on drop

The H1 test showed that ALL FOUR together fail in release, ALL FOUR
together pass in debug. The culprit is not any individual function but
the aggregate impact of cross-module function calls on LLVM's optimization
of the enclosing ~4,500-line function.

## Reference: log bundles from session 2 (deleted 2026-04-24)

The following diagnostic log bundles were preserved during the investigation
and deleted after the gate landed. All carried `commit_dirty: true` — none
were reproducible from a committed hash alone. Listed here for the record:

| Path | What it was |
|---|---|
| `logs/h2-unregistered-42/` | Gate code present, not registered. Reference-good state. |
| `logs/h1-diagnostic-42/` | Registered gate + `DispositionKind::Resting => false` proxy. Failed. |
| `logs/merge-broken-42/` | Gate inlined into resolve_goap_plans with `continue`. Failed hard. |
| `logs/merge-nopush-42/` | Gate inlined, `continue` removed. Failed. Same shape. |
| `logs/tuned-42/` | Probe 3 partial (killed). Empty-body gate. Passed canaries during its partial window. |
| `logs/c-fixed-focal/` | First-attempt draft (session 1). |
| `logs/disable-gate-42/` | Pre-session-2 no-gate baseline (different commit, stale). |
