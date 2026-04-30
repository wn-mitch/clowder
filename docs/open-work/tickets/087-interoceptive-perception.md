---
id: 087
title: Interoceptive perception — agents need a structured self-snapshot, not raw component reads
status: in-progress
cluster: ai-substrate
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Cats build a cohesive model of the *external* world via a per-tick perception layer that authors ZST markers and scalars consumed by IAUS DSEs (`src/systems/sensing.rs::update_target_existence_markers` and siblings). They build no equivalent model of the *self*. Each DSE that needs body-state information either reaches into raw `Needs`/`Health` fields directly or relies on `ScoringContext` population (`src/systems/disposition.rs:730–895`) to do ad-hoc raw reads. There is no perception module that publishes "the cat's view of its own body" the way `update_target_existence_markers` publishes the cat's view of nearby world objects.

User-visible symptom: critical-health interrupts (`src/systems/disposition.rs:263`) fire frequently, and the post-interrupt replan picks Guarding/Crafting on jitter rather than Flee or Rest (ticket 047 treadmill). Root cause: only Sleep currently reads `health_deficit` as a scoring axis. At health 0.5 nothing elevates self-care; at health 0.39 the interrupt fires reactively into a score landscape that still doesn't favor recovery. There is no `Rest` DSE distinct from `Sleep`; Sleep keys on `energy_deficit` and day-phase (with a smaller injury contribution), conflating fatigue and injury-recovery semantics.

The fix is structural, not a per-DSE patch: add an **interoceptive perception module** participating in IAUS exactly like external perception modules — same Chain 2a registration, same marker/scalar publishing pattern, same DSE consumption shape — and migrate body-aware DSEs (and `ScoringContext` population) to consume from it.

Cross-links 047 (critical-health treadmill), 076 (last-resort promotion modifier — parked), 032 (body-condition welfare-axis). The cluster reframes as a perception gap rather than three independent fixes.

## Approach

Symmetry rule: every architectural choice mirrors the existing external-perception module. ZST markers in `src/components/markers.rs` with `KEY` constants; authoring system scheduled in Chain 2a in `SimulationPlugin::build()` before GOAP; scalars centralized in `ctx_scalars()` in `src/ai/scoring.rs`; DSE consumption via `EligibilityFilter::require/forbid(KEY)` and named scalar reads. No new pattern.

What it publishes (Phase A, additive):

- Markers: `LowHealth` (gate at `critical_health_threshold` so the perception fires *before* the interrupt), `SevereInjury` (one or more `Injury` with `severity > severe_injury_threshold`), `BodyDistressed` (composite — any of {hunger_urgency, energy_deficit, thermal_deficit, health_deficit} above `body_distress_threshold`). The existing `Injured` marker (`src/components/markers.rs:119`) stays at its current authoring site; the new module adds the three above.
- Scalars: `health_deficit` (moves from raw read in `ctx_scalars` to perception-backed), `pain_level` (new — derived from `Health.injuries.iter().map(|i| i.severity).sum()` normalized by `pain_normalization_max`), `body_distress_composite` (new — max or weighted-max of normalized body-state urgencies; the unified "I am unwell" signal).
- No new anchors yet (defer to 089).

Consumers (Phase A):

- `Flee` DSE (`src/ai/dses/flee.rs`): adopt `health_deficit` as a fourth Consideration in its `CompensatedProduct`. CompensatedProduct gating is the right composition because we want low health to *gate* fleeing (you flee harder when wounded), not merely add to it. Use `flee_or_fight(critical_health_threshold)` to land the inflection at the same point the interrupt cares about. Composition vector grows from `vec![1.0, 1.0, 1.0]` to `vec![1.0, 1.0, 1.0, 1.0]` (CP weights are neutral 1.0 by convention).
- New `Rest` DSE — `src/ai/dses/rest.rs`. Self-state DSE following Sleep's structural pattern. WeightedSum: `health_deficit` (highest weight), `pain_level`, inverted `safety_deficit`. Eligibility: `forbid(InCombat::KEY)` + `forbid(HasThreatNearby::KEY)`. Maps to a Resting disposition that the critical-health interrupt at `disposition.rs:263` already special-cases by *not* re-interrupting; this gives the post-interrupt replan a deterministic winner instead of Guarding/Crafting jitter.
- `ScoringContext` population in `src/systems/disposition.rs:730–895`: replace direct `health.current` / `needs.*` reads with reads off the interoceptive surface. This is the leak-fix that closes the architectural asymmetry — the real point of the work.
- `Sleep` DSE: no signature change; its existing `health_deficit` term is now perception-sourced.

What stays out of Phase A (now its own ticket):

- 088 — `body_distress` Modifier under §L2.10 Modifier substrate.
- 089 — Self-anchors (`OwnInjurySite`, etc.).
- 090 — L4/L5 self-perception scalars (mastery / purpose / esteem).
- The persistent `BodyMemory` trend component overlaps 032's body-condition welfare-axis. If 032 lands first this folds into it; if 087 lands first, 032 reframes as a consumer of the interoceptive surface.

## Scope

Files added:
- `src/systems/interoception.rs` — new perception module mirroring `update_target_existence_markers`. Authors `LowHealth` / `SevereInjury` / `BodyDistressed`. No resource output.
- `src/ai/dses/rest.rs` — new self-state DSE structured after `src/ai/dses/sleep.rs`.

Files modified:
- `src/components/markers.rs` — add `LowHealth`, `SevereInjury`, `BodyDistressed` ZST markers with `KEY` constants under the State markers section.
- `src/ai/scoring.rs` (`ctx_scalars`) — add `pain_level`, `body_distress_composite` keys; rewire `health_deficit` to source from interoceptive surface.
- `src/systems/disposition.rs` (`ScoringContext` population) — replace direct `health` / `needs` reads with reads from interoceptive markers/scalars.
- `src/plugins/simulation.rs` — register `interoception::author_self_markers` in Chain 2a alongside `update_target_existence_markers`. Register `Rest` DSE in `populate_dse_registry`.
- `src/ai/dses/flee.rs` — add `health_deficit` Consideration to `CompensatedProduct`; grow composition vector to length 4.
- `src/resources/sim_constants.rs` — add `severe_injury_threshold`, `body_distress_threshold`, `pain_normalization_max` knobs; serialize into `events.jsonl` header automatically per §"Tuning constants".
- `src/resources/system_activation.rs` — no new positive `Feature::*` expected (this is substrate, not a new behavior).

Step contract compliance: no new step resolvers in Phase A. `scripts/check_step_contracts.sh` linter unaffected.

## Verification

- `just check && just test` — both green.
- `just soak 42 && just verdict logs/tuned-42`. Required:
  - `deaths_by_cause.Starvation == 0` (hard gate, unchanged).
  - `interrupts_by_reason.CriticalHealth` materially lower than baseline. The whole point: interrupts become rare, not load-bearing.
  - All six continuity canaries ≥1.
  - `never_fired_expected_positives == 0`.
- `just soak-trace 42 <focal-cat>` on a cat that took damage. Confirm Flee or Rest *won the disposition score* below `critical_health_threshold + ~0.1` instead of the cat staying in Guarding/Crafting until the interrupt fired.
- `just frame-diff` against pre-change baseline: Flee and Rest scores show |Δ mean(final_score)| up; Sleep stable.
- Drift > ±10% on any characteristic metric requires the four-artifact hypothesis per CLAUDE.md. Pre-stated hypothesis: *interoceptive perception elevates Flee and Rest scoring at low health → critical-health interrupts decrease (~50%+), starvation deaths unchanged, ShadowFoxAmbush deaths flat or down, total deaths flat or down.* Run `just hypothesize` if any characteristic metric drifts.
- `events.jsonl` header carries the three new SimConstants knobs; runs remain comparable iff headers match.

## Out of scope

- Modifier-level distress promotion → 088.
- Spatial self-anchors → 089.
- L4/L5 self-perception → 090.
- Persistent `BodyMemory` trend component → 032 / coordination noted above.

## Log

- 2026-04-30: Opened. Cluster reframed as a perception gap rather than three independent fixes (047 / 076 / 032). Phase B sub-tickets 088 / 089 / 090 opened in the same commit, all `blocked-by: [087]`.
