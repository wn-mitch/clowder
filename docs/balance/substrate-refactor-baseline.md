# AI Substrate Refactor — Pre-Refactor Baseline

**Status:** archived 2026-04-21 as the diff target for every phase of the AI
substrate refactor (`docs/systems/refactor-plan.md`). Not a balance iteration
— a snapshot of current-tip behavior the refactor is expected to drift
*from* in predicted directions.

## Contract

Per `docs/systems/refactor-plan.md` pre-flight gate 6:

> Archive a baseline soak at the commit that clears gates 1–5. `just soak
> 42` → `logs/baseline-pre-substrate-refactor/`. Keep `events.jsonl`,
> `narrative.jsonl`, and `header.constants` versioned here. This is the
> diff target for every phase.

Every frame-diff run emitted by `just frame-diff` (landing in Phase 1) compares
against this baseline. Drift in predicted directions advances the phase;
wrong-direction drift is a rejection.

## Run parameters

| Field | Value |
|---|---|
| Seed | 42 |
| Duration | 900s (15-min canonical deep-soak) |
| Build | release (`cargo build --release`) |
| Binary | `./target/release/clowder --headless` |
| Output | `logs/baseline-pre-substrate-refactor/{events,narrative}.jsonl` |

## Commit anchor

**Parent commit:** `333fd7b` — `docs: AI substrate refactor second-pass spec
+ instrumentation layer` (2026-04-21).

**Working-copy state at baseline capture:** `commit_dirty: true`. Pre-flight
gates 1–5 all landed as working-copy edits, not yet committed at archive
time:

- Gate 1 — `docs/open-work.md`, `docs/balance/activation-1-status.md`
  parking notes.
- Gate 3 — `tests/integration.rs` resource + component fixtures.
- Gate 4a — `needs.warmth` → `needs.temperature` field rename across 14
  files.
- Gate 4b — 46 `*_warmth_*` constants renamed to `*_temperature_*` in
  `src/resources/sim_constants.rs`, plus call-site updates and the planner
  `WarmthOk` / `warmth_ok` → `TemperatureOk` / `temperature_ok` rename.
- Gate 5 — `fox_softmax_temperature: 0.15` added to `ScoringConstants`.

**Implication for reproducibility:** per `CLAUDE.md` "Simulation
Verification", headers with `commit_dirty: true` cannot be reproduced from
the commit alone. However, **all phase frame-diffs will be run against this
same dirty tree** (or its successor commits), so the baseline and its
comparison frames are internally comparable. This is the same convention
`docs/balance/eat-inventory-threshold.report.md` documents for its dirty-tree
baseline/treatment pair.

**Re-baseline trigger:** once the pre-flight working tree lands as a proper
commit, re-run `just soak 42` under that clean commit hash and replace the
artifacts in `logs/baseline-pre-substrate-refactor/`. Update this doc's
**Commit anchor** section with the new hash.

## Inherited regression (parked)

`activation-1-status.md` documents the founder-age regression
(`start_tick = 60 × ticks_per_season` spawns founders near end-of-life;
baseline-5b-v2 showed 15/15 colonies wipe before day 180). Per
`docs/systems/refactor-plan.md` pre-flight gate 2 decision, that regression
is **parked** — it's a standalone balance iteration scheduled for
post-Phase-7.

**Baseline soak inherits this wipeout tendency.** Every frame-diff run
against this baseline until a post-refactor re-baseline must account for
the noise this introduces. If the baseline soak itself wipes out before
day 180 (a seed-42 expectation per the activation-1 evidence), the
footer tallies below reflect a partial-colony run rather than a
full-window run.

## Dormancy snapshot (target for positive-exit criteria)

The refactor's refactor-level positive-exit criteria
(`docs/systems/refactor-plan.md` §"Positive exit criteria") require these
metrics to move from approximately-zero to measurably nonzero:

| Metric | Baseline (pre-refactor) | Target (post-Phase-7) |
|---|---|---|
| Farming fires | 0 (never) | ≥1 per seed-42 soak |
| `MateWithGoal` fires | ~0 (gate-starved) | ≥3 per seed-42 soak |
| Kittens surviving | ~0 | ≥2 per starter colony |
| Crafting recipes completed | sparse | non-zero, progress legible |
| PracticeMagic sub-mode diversity | sparse | 5 sub-modes each ≥1× |
| Build / Mentor / Aspire frequency | low | rises vs baseline |
| The Calling fires | 0 (not implemented) | ≥1 Named Object per sim year |
| Warring-self signal instances | 0 (no register) | ≥1 documented per soak |

Values in the "Baseline" column get filled from the archived
`events.jsonl` footer in the **Baseline metrics** section below.

## Baseline metrics

Captured from `logs/baseline-pre-substrate-refactor/events.jsonl` footer
on the 2026-04-21 soak (working tree dirty; see Commit anchor above).

| Metric | Value |
|---|---|
| Colony survived to end of 900s | **yes** — headless reached sim day 1380 (started day 1201 → 179 sim-days) without a wipeout-early exit. |
| Schedule runs | 179,230 |
| Narrative entries | 456,432 |
| Event entries | 944,771 |
| `Starvation` deaths | **3** (expected: pre-existing canary failure per `docs/balance/eat-inventory-threshold.report.md`; not a new regression) |
| `ShadowFoxAmbush` deaths | **0** (pass — target ≤ 5) |
| Other deaths by cause | (none) |
| `positive_features_active / total` | 17 / 33 |
| `neutral_features_active / total` | 13 / 20 |
| `negative_events_total` | 179,447 |
| `shadow_fox_spawn_total` | 0 |
| `anxiety_interrupt_total` | 0 (all interrupts routed through the `interrupts_by_reason` counter instead) |
| `wards_placed_total` / despawned / final | 435 / 433 / 2 |
| `ward_avg_strength_final` | 0.784 |
| `ward_siege_started_total` | 0 |
| Interrupts (top) | `CriticalSafety preempted level 4 plan` × 30; `CriticalSafety preempted level 5 plan` × 3 |
| Plan failures (top 5) | `EngagePrey: lost prey during approach` × 3,516; `ForageItem: nothing found while foraging` × 2,482; `EngagePrey: stuck while stalking` × 451; `EngagePrey: seeking another target` × 329; `EngagePrey: prey teleported` × 208 |

### Canary summary

`scripts/check_canaries.sh logs/baseline-pre-substrate-refactor/events.jsonl`:

```
[FAIL] starvation_deaths                3 (target == 0)
[pass] shadowfox_ambush_deaths          0 (target <= 5)
[pass] footer_written                   1 (target >= 1)
[pass] features_at_zero                 0 (target informational)
```

The `starvation_deaths` fail is a **pre-existing condition** inherited by
this baseline, not a regression introduced by pre-flight gates 1–5. Post-
Phase-7 balance work resolves it per the plan; until then, every phase's
frame-diff must treat a non-decreasing Starvation count as expected
baseline noise.

### Wipeout observation

`activation-1-status.md` flagged a pre-existing colony-survival regression
where 15/15 seed-sweep colonies wiped before day 180. **This seed-42
baseline did not wipe** on the post-rename working tree: the colony ran
the full 900s wall window, ending at sim day 1380. This is consistent
with the prior observation that seed 42 occasionally survives longer than
other seeds despite the founder-age pressure. Multi-seed sweeps at phase
exit (per the plan's verification loop) will surface whether the wipeout
pattern persists under other seeds.

## Characteristic-metric drift tolerance

Per `CLAUDE.md` Balance Methodology, drift ≤ ±10% on characteristic
metrics is measurement noise. Drift > ±10% requires the four-artifact
rule (hypothesis / prediction / observation / concordance). Drift > ±30%
requires scrutiny before acceptance.

For the substrate refactor, **drift in the direction of the refactor
hypothesis is the goal**, not noise to suppress. Each phase's
`docs/balance/substrate-phase-N.md` declares its per-DSE predictions;
concordance is gated per phase.

## Artifact paths

- `logs/baseline-pre-substrate-refactor/events.jsonl` — machine-readable
  event log with header, per-tick events, and footer tallies.
- `logs/baseline-pre-substrate-refactor/narrative.jsonl` — tiered
  narrative log (Micro / Action / Significant / Danger / Nature).
- `logs/baseline-pre-substrate-refactor/stderr.log` — headless runner
  stderr (seed confirmation, wipeout warnings if any, footer echo).

## Cross-refs

- `docs/systems/refactor-plan.md` pre-flight gate 6 (this doc's contract).
- `docs/systems/ai-substrate-refactor.md` — refactor spec the plan
  implements.
- `CLAUDE.md` "Simulation Verification" — canonical deep-soak convention.
- `docs/balance/activation-1-status.md` — parked regression the baseline
  inherits.
- `docs/balance/eat-inventory-threshold.report.md` — reference pattern
  for dirty-tree balance reports.
