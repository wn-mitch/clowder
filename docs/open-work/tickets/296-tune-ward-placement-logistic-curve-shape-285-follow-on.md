---
id: 296
title: tune ward placement Logistic curve shape (285 follow-on)
status: ready
cluster: balance
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [284-ward-anchor-tuning.md]
landed-at: null
landed-on: null
---

## Why
285's three-seed triangulation (42, 99, 7) proved that the `(0.5, 0.3) → (0.7, 0.4)` symmetric scale-up on the ward anchor weights produces byte-identical or near-byte-identical ward placements across all three seeds — even though `shadow_foxes_avoided_ward_total` spans 2 → 78 (39× spread) across those topologies, proving the metric is sensitive but the magnitude lever is not. The suspect binding constraint is the Logistic curve `L(x) = 1 / (1 + exp(-8(x - 0.5)))` (steepness=8.0, midpoint=0.5) applied to both `RecentAmbushMap` and `CarcassScentMap` samples in `compute_ward_placement()`: it saturates near 1.0 on any ambush-hot or carcass-anchored tile at weights ≥ ~0.3, so weight magnitude past saturation produces no ordering change in the placement scorer. The lever is curve shape, not magnitude — softer steepness or a shifted midpoint would re-introduce per-tile gradient that the weights could then bias.

## Scope
- Run `just hypothesize` four-artifact sweeps over `compute_ward_placement`'s Logistic steepness and midpoint. Curve constants are not currently exposed as `SimConstants` fields — adding them is part of this ticket's surface (see `src/systems/coordination.rs:1390-1502`).
- Candidate steepness values: `4.0` (softer), `2.0` (very soft), keep `8.0` as baseline. Candidate midpoint values: `0.3`, `0.5` (baseline), `0.7`. Pre-register predictions per shape; iterate per concordance verdict.
- Land the winning curve-shape values + update `compute_ward_placement` doc-comments + append `docs/balance/284-ward-anchor-tuning.md` iter-3.

## Out of scope
- Adding new perception axes / substrate sources — 297's surface.
- Re-tuning the anchor *weights* (`ward_ambush_anchor_weight`, `ward_recency_anchor_weight`); 285's three-seed result is conclusive that magnitude is inert at this curve shape. Re-test only AFTER curve shape changes.
- Cleansing / banishment / reactive ward removal.

## Current state
220 landed the Logistic-curve substrate with hardcoded `(steepness=8.0, midpoint=0.5)` in `compute_ward_placement()`. 284 lifted the weights off `0.0` to `(0.5, 0.3)` as a first-light activation. 285 (iter-2 in `docs/balance/284-ward-anchor-tuning.md`) ran the four-artifact magnitude sweep across three seeds and surfaced this curve-shape constraint as the architectural binding.

## Approach
1. Expose `ward_placement_logistic_steepness` and `ward_placement_logistic_midpoint` as `SimConstants` fields, defaulting to the current hardcoded values. Substrate stub allowlist not required since this is a value-extraction refactor, not a marker addition.
2. Add unit tests guarding "current shape preserved when defaults are forced" (regression).
3. Write `docs/balance/hypothesis-296-curve-shape.yaml` predicting that softer steepness + lower midpoint lifts `shadow_foxes_avoided_ward_total` on seed-7 (where the metric has headroom at 78).
4. Run `just hypothesize`; iterate per concordance band per the 285 plan template in `.claude/plans/work-285-floofy-frog.md`.

## Verification
- `just hypothesize <spec>` exits concordant.
- Treatment soak: `just soak-trace 42 Wren` + `just verdict` shows continuity canaries hold; ward placement positions visibly shift relative to 285 iter-2's three-seed snapshot.
- Append iter-3 to `docs/balance/284-ward-anchor-tuning.md` documenting the curve-shape landing.

## Log
- 2026-05-12: opened as the curve-shape follow-on to 285. Architectural read in 284 iter-2 §"Seed-99 + seed-7 retries" identifies the Logistic curve as the binding constraint after three-seed triangulation.
