---
id: 416
title: Starvation rebalance — promote graded-cliff + body-condition defaults to ship behavior
status: ready
cluster: life-cycle
initiative: [welfare-fidelity]
added: 2026-05-18
parked: null
blocked-by: []
supersedes: []
related-systems: [needs.md]
related-balance: [starvation-rebalance.md, routing-the-mating-gate-through-the-body-condition-welfare-a.md]
landed-at: null
landed-on: null
---

## Why

Surfaced as a follow-on of ticket 032 (Starvation rebalance — align with IRL
cat biology). 032 shipped the substrate scaffolding for Items 1/2/5 (graded
cliff, per-life-stage multipliers, body-condition welfare axis) and the
treatment-side sweeps are passing per balance threads. But the ship defaults
in `src/resources/sim_constants.rs` remain LEGACY:

- `starvation_cliff_use_legacy: true` (line 270) — cats still take the old
  always-on `(1 − hunger)^k` damage curve, not the threshold-gated ramp
  designed in 032 Item 1.
- `use_body_condition_for_breeding_gate: false` (line 6763) — Item 5's
  body-condition welfare axis ships inert; the mating gate still reads the
  raw hunger floor.

032 explicitly deferred the default-flip to a follow-on once upstream
regressions cleared (032 Iter 5 verdict, 2026-05-03). Those regressions are
now cleared: the 148 diagnostic confirms canonical seed-42 courtship holds
(1695+ events / 900s); the 187 kitten-starvation cascade closed via 398;
Starvation deaths held at 0 across recent baselines.

This ticket flips the defaults and re-verifies survival + continuity
canaries against the current canonical baseline.

## Scope

- Flip `starvation_cliff_use_legacy` default `true → false` in
  `src/resources/sim_constants.rs`.
- Flip `use_body_condition_for_breeding_gate` default `false → true`.
- Re-run `just soak 42` against the new defaults; run `just verdict` against
  the resulting `logs/tuned-42/`.
- If the verdict fails any continuity canary, identify which axis caused the
  regression and either tune (within scope) or revert that single flip
  (out-of-scope of the rest).

## Out of scope

- Re-running the treatment sweeps already on disk. The sweep evidence is
  sufficient — what remains is the canonical-baseline gate.
- Item 2 default for per-life-stage multipliers (`life_stage_starvation_mult_kitten` etc.)
  if these don't have a legacy/treatment flag distinction; verify their
  current defaults match the Item-2 sweep treatment values before flipping
  the other two.

## Current state

Substrate code in main as of 032's Iter-5 landing (commit 2026-05-03
landings). Sweeps logged at:
- `logs/sweep-threshold-gated-graded-cliff-drain-only-below-hunger-0-15-pa-treatment/`
- `logs/sweep-per-life-stage-starvation-multipliers-kitten-2-0-young-1-3-a-treatment/`
- `logs/sweep-routing-the-mating-gate-through-the-body-condition-welfare-a-treatment/`

Item-5 verdict: concordant, +43.2% courtship_tally vs baseline.

## Approach

1. Flip the two booleans in `sim_constants.rs`.
2. `just soak 42` → `just verdict logs/tuned-42` → ensure pass.
3. Append a closing iteration to `docs/balance/starvation-rebalance.md`
   capturing the canonical-baseline numbers post-flip (deaths_by_cause,
   continuity_tallies, footer comparison vs current canonical baseline).
4. If verdict passes: `just land 416`.

## Verification

- `deaths_by_cause.Starvation == 0` (hard gate).
- `deaths_by_cause.ShadowFoxAmbush <= 10` (hard gate).
- Continuity canaries (grooming, play, mentoring, courtship, mythic-texture)
  all ≥ 1 per soak.
- `continuity_tallies.courtship` does not regress below the pre-flip
  baseline (canonical baseline check).
- `KittenMatured` activation continues to fire (generational continuity).

## Log

- 2026-05-18: Opened as follow-on of 032. The 032 ticket landed with
  substrate scaffolding + treatment-side sweeps documented; default-flip
  was deferred behind the courtship-canary regression (since cleared).
  This ticket completes the actual rebalance by promoting the new
  behavior to ship.
