# Phase 6a §7 commitment gate — attempt status

> **Status (2026-04-23 evening):** **Deferred after second unresolved attempt.** Working-copy state is H2-equivalent: all helpers + tests + per-DSE strategy tags + `Feature::CommitmentDropTriggered` are present, but the gate is not wired into any schedule or execution loop. Soak canaries parity-with-main (H2 measurement: Starvation=0, grooming=189, wards_placed=233).

This doc captures the second-attempt deep dive so a future session can pick it up without re-deriving the bisection. See also:

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

## Unresolved mystery

At the point of handoff, the next bisection step was about to compare:

- **Empty-body probe** (above): `let _ = plan.kind;` etc. — **passes**.
- **Inline strategy lookup**: add `let _ = crate::ai::commitment::strategy_for_disposition(plan.kind);` only. This was the next diagnostic to run; soak not yet attempted.

The four candidate culprits:

1. `strategy_for_disposition(plan.kind)` — pure match on enum, returns Copy value. Only difference from the passing probe is a function-call boundary.
2. `proxies_for_plan(&plan, &needs, d)` — takes two shared refs. The **only** call that passes `&plan` or `&needs`. Hypothesis-of-leading: the `&Mut<GoapPlan>` → `&GoapPlan` deref coercion trips something in Bevy's change-detection or access-tracking.
3. `should_drop_intention(strategy, proxies)` — pure match on two Copy types. No refs.
4. `record_drop(narr.activation.as_deref_mut())` — fires only on drop (~7× in a merge run). Rare enough that its direct impact is unlikely.

The "inline field reads pass; function calls fail" shape suggests:

- **LLVM/codegen**: calling into another crate module may force the optimizer to reload from memory what it would otherwise keep in registers, shifting instruction timing. This can interact with Bevy's change-detection tick counters if they're compared with tick values read at different times.
- **Bevy `Mut<T>` semantics under coercion**: specifically worth checking whether `&plan` (where `plan: Mut<'_, GoapPlan>`) coerces to `&GoapPlan` via a route that actually invokes `DerefMut` briefly (e.g., if the reborrow mechanism in 0.18 does a mutable touch).
- **A genuine Rust/Bevy interaction I'm not seeing** — for a session 3 reader, don't assume this section is exhaustive; the bisection was cut short.

## What to try next

When picking this up:

1. **Do not re-derive.** Read this doc end-to-end first.
2. **Start the bisection at step 1:** add exactly `let _bisect = crate::ai::commitment::strategy_for_disposition(plan.kind);` to the empty-body probe. Soak seed-42 Calcifer. If canaries hold, the function-call boundary itself isn't the issue — move to step 2.
3. **Step 2:** replace with `let _bisect = crate::ai::commitment::proxies_for_plan(&plan, &needs, d);` (skip strategy lookup). This is the high-probability culprit based on the access pattern.
4. **If proxies_for_plan fails:** replace its body with a no-op returning a constant `BeliefProxies`. If that passes, it's the *reads inside* the function (not the call boundary). Instrument the field reads inside with `black_box` to rule out optimization effects.
5. **If step 2 passes:** proceed to `should_drop_intention` and `record_drop` in turn. Whichever fails is the culprit.
6. **Landing criterion:** once the guilty call is isolated, either rework the signature to avoid the offending pattern, or document the constraint and pursue an alternative integration (e.g., route the drop decision through a post-processing pass on a separate component instead of a mid-loop function call).

## Resumption checklist

- [ ] Read this doc.
- [ ] Read the CLAUDE.md §7.2 mental-model subsection for the gate's semantic contract.
- [ ] Read `src/ai/commitment.rs` — the helpers are clean; the tests cover the 12-row strategy table and the Resting trip-guard.
- [ ] Run `just autoloop 42 Calcifer` on the current working copy to confirm H2-parity (this is the baseline to beat — Starvation=0, grooming ~189, wards_placed ~233).
- [ ] Start at bisection step 1 above.

## Reference: log bundles preserved from session 2

| Path | What it is |
|---|---|
| `logs/h2-unregistered-42/` | Gate code present, not registered. **Reference-good state.** |
| `logs/h1-diagnostic-42/` | Registered gate + `DispositionKind::Resting => false` proxy. Failed. |
| `logs/merge-broken-42/` | Gate inlined into resolve_goap_plans with `continue`. Failed hard. |
| `logs/merge-nopush-42/` | Gate inlined, `continue` removed. Failed. Same shape. |
| `logs/tuned-42/` | Probe 3 partial (killed). Empty-body gate. Passed canaries during its partial window. |
| `logs/c-fixed-focal/` | First-attempt draft (session 1), preserved for reference. |
| `logs/disable-gate-42/` | Pre-session-2 no-gate baseline (different commit, stale). |

The `commit_dirty: true` flag is set on all of these — none are reproducible from a committed hash alone.
