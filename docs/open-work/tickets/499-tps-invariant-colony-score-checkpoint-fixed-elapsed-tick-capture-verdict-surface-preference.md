---
id: 499
title: TPS-invariant colony-score checkpoint (fixed elapsed-tick capture + verdict surface preference)
status: ready
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
initiative: []
added: 2026-06-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
`ColonyScore.aggregate = welfare × max(1, seasons_survived) +
achievement_points + positive_activation_score` is read at end-of-run,
but soaks are fixed **wall-clock** (900 s) — so both the seasons
multiplier and the achievement ledger grow with elapsed sim-time, and
the score conflates colony health with **binary throughput**. The 480
regression (197 → 72 t/s) deflated end-of-run aggregate ~30% with
identical per-tick behavior; the historical 1330 → 800–1050 trajectory
partly charts binary speed. Score-chasing against that number optimizes
the wrong thing. Full assessment:
`docs/balance/colony-score-metric-assessment.md`.

## Scope
- `ColonyScoreConstants::checkpoint_elapsed_ticks = 50_000` (2.5
  seasons; 10k ticks clear of integer-season boundaries; ~20% under the
  slowest current run's ~63k elapsed, so every current binary reaches it
  in 900 s; 0 disables).
- `ColonyScore.run_start_tick` (seeded in `build_new_world` beside
  `last_recorded_season` — ticks on disk are absolute, ≈1.2M) +
  `checkpoint: Option<ColonyScoreCheckpoint>` freezing the snapshot
  **and the achievement ledger** (the other elapsed-time-dependent term)
  exactly once, at the first `emit_colony_score` emission at/after the
  mark.
- Footer block `colony_score_at_checkpoint` (null when never reached;
  carries `captured_at_elapsed_tick` + `checkpoint_constant`). End-of-run
  block unchanged.
- `scripts/verdict.py::select_colony_score_blocks` — verdict prefers the
  checkpoint surface when BOTH runs carry it at the SAME constant, labels
  the surface (`checkpoint` / `end_of_run`), and flags end-of-run
  fallbacks as TPS-confounded in `next_steps`.

## Out of scope
- Redefining the aggregate formula (125's boundary stands; integer-season
  jumpiness and point-in-time welfare are accepted residuals — see the
  assessment doc).
- AUC / per-season-delta machinery in the sim — computable post-hoc from
  the `ColonyScore` event stream in tooling if ever needed.

## Current state
Implemented in this session (instrumentation wave with 498 + 490's
dispersion canary). **Post-checkpoint scores are a new series — not
comparable to 1330-era end-of-run numbers.** First baseline carrying the
block promotes at the end of this session.

## Approach
Two-commit shape per ticket 125: Rust capture + footer block, then
verdict surface preference. Capture lives in
`src/systems/colony_score.rs::emit_colony_score` (emission granularity
100 ticks; actual capture tick recorded).

## Verification
- Unit: `tests/verdict/test_colony_score_drift.py`
  `TestCheckpointSurfaceSelection` (preference, legacy fallback,
  early-death fallback, constant mismatch).
- Integration: `tests/integration.rs::footer_carries_throughput_checkpoint_and_dispersion_surfaces`
  (block present, null before the mark).
- `just soak 42` + `just verdict`: surface label `end_of_run` against a
  legacy baseline; `checkpoint` after re-promote.

## Log
- 2026-06-09: opened + implemented same session. 50k mark justified in
  the constant's doc comment; assessment doc records the
  effective-as-lens / ineffective-as-target verdict.
