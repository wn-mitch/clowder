---
id: 075
title: CommitmentTenure Modifier (anti-oscillation hysteresis on disposition switching)
status: blocked
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: [072]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Audit gap #5 (important severity). Only `BeingBefriended` has hysteresis in the codebase today (`sim_constants.rs:672–674`, `befriend_familiarity_hysteresis`). No commitment-tenure exists on disposition switching: when two dispositions score within ε of each other, the cat oscillates every tick. Oscillation defeats single-disposition commitment, churns plans, and wastes the IAUS's score economy.

Lands as a Modifier in the §3.5.1 pipeline so anti-oscillation falls out of "current disposition wins ties" — additive lift on the incumbent's score makes it the natural IAUS pick during the tenure window. **No external switch-gate, no override of the IAUS pick.** Inspectable in the same modifier-pipeline trace as `Pride` / `IndependenceSolo` / `Patience`.

Parent: ticket 071. Blocked by ticket 072 (needs `plan_substrate::record_disposition_switch` to write `disposition_started_tick`).

## Scope

- Add `CommitmentTenure` modifier to `src/ai/modifier.rs` alongside `Pride` (line 132), `IndependenceSolo` (190), `Patience` (303), `Tradition` (432), `FoxTerritorySuppression` (500).
- Trigger: sensor `tick - disposition_started_tick < min_disposition_tenure_ticks` via `plan_substrate::COMMITMENT_TENURE_INPUT`.
- Effect: additive lift `oscillation_score_lift` on the cat's *current* disposition's score. The IAUS picks the highest-scored disposition; on a tie, the incumbent wins.
- New `DispositionConstants` knobs: `min_disposition_tenure_ticks: u64` (default ~200 ≈ 30 sim-minutes), `oscillation_score_lift: f32` (default 0.10).
- Register `CommitmentTenure::new(sc)` at the modifier-registration site (alongside `Pride::new(sc)` callers in plugin construction).

## Out of scope

- Tuning `min_disposition_tenure_ticks` or `oscillation_score_lift` — pick conservative defaults; tune via post-landing sensitivity sweep.
- Hysteresis on action selection within a single disposition (the disposition layer is the right level).
- Hysteresis on target selection (different concern; addressed by 073's cooldown).

## Approach

Files:

- `src/ai/modifier.rs` — add `CommitmentTenure` modifier struct + `new(sc: &ScoringConstants) -> Self` constructor (model after `Pride` at line 132). Implements the same trait as the existing modifiers; reads `COMMITMENT_TENURE_INPUT` sensor; applies additive lift to the cat's current disposition score.
- `src/ai/scoring.rs::EvalInputs` — publish the `commitment_tenure_progress` sensor: returns `(tick - disposition_started_tick).min(min_tenure_ticks) as f32 / min_tenure_ticks as f32`. The modifier consults this directly; or it can read `disposition_started_tick` off `DispositionState` and compute internally.
- `src/resources/sim_constants.rs::DispositionConstants` — add `min_disposition_tenure_ticks: u64` and `oscillation_score_lift: f32`.
- Modifier registration site (search for `Pride::new(sc)` callers in plugin construction — likely `src/plugins/simulation.rs` or `src/ai/scoring.rs`'s registration section) — add `CommitmentTenure::new(sc)` to the registered modifier list.

## Verification

- `just check && just test` green.
- Unit test: `CommitmentTenure::compute` returns the configured `oscillation_score_lift` while `tick - disposition_started_tick < min_tenure_ticks`, returns 0.0 outside the tenure window.
- Unit test: the modifier targets only the cat's current disposition (other dispositions get 0.0 lift).
- Disposition-oscillation synthetic test: two dispositions tied within ε of each other → cat stays on its current disposition for `min_disposition_tenure_ticks` before switching. Without the modifier, the same setup oscillates every tick (regression-test framing).
- `just soak 42 && just verdict logs/tuned-42-075` — hard gates pass; expect minor stickiness drift (cats stay in current disposition longer; mean disposition-tenure rises).

## Log

- 2026-04-29: Opened under sub-epic 071.
