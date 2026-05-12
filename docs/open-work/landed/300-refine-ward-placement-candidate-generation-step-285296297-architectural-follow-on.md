---
id: 300
title: refine ward placement candidate-generation step (285+296+297 architectural follow-on)
status: done
cluster: balance
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [284-ward-anchor-tuning.md, 297-fox-patrol-topology-axis.md]
landed-at: pending
landed-on: 2026-05-12
---

## Why

285 + 296 + 297 collectively ruled out three threat-axis levers (magnitude, curve shape, new orthogonal axis) as placement levers — six independent constant changes on three seeds (42, 99, 7), placement argmax byte-identical on every one. Architectural finding documented in `docs/balance/297-fox-patrol-topology-axis.md` iter-2.

In every soak across the 285+296+297 chain, the seven unique ward positions in `WardPlaced` events all sit at multiples of 5: `(29,23), (33,10), (33,22), (38,22), (39,23), (42,36), (62,3)`. Every coordinate is `5k` — not by colony geometry, by candidate-generation construction. The "optimal" placement tile might be off-grid (e.g., `(30,52)` instead of `(30,50)`, or `(33,21)` instead of `(33,22)`) but `compute_ward_placement` literally cannot score it.

Among the four non-threat-axis levers 297 iter-2 catalogued (`cat_value` coefficient — 298, `distance_cost` — 299, candidate-step — *this ticket*, decision semantics — 301), this is the most §4.7-clean: not a substrate change, not a scoring-semantics change, just a search-grid refinement. And it's the cheapest standalone test — one promoted constant, one hypothesize spec, one regression run.

## Scope

1. Promote `CANDIDATE_STEP` (`src/systems/coordination.rs:1421`) to `SimConstants` as `ward_placement_candidate_step` with `#[serde(default = ...)]`, default 5 — preserve byte-identical pre-change behavior at the default.
2. `just hypothesize` sweep on seed-42 across `{2, 3, 5 baseline, 1}`. Predict the metric may move because the scorer can now consider intermediate tiles.
3. If seed-42 moves, triangulate seeds 99 and 7 (same load-bearing seeds as 285 / 296 / 297).
4. Balance writeup as `docs/balance/300-ward-candidate-step.md` following the four-artifact template; promote to iter-2 if seed-42 lifts.
5. Cost analysis: step=5 → ~430 candidates; step=2 → ~2700 (6×); step=1 → ~10800 (25×). Verify scorer wall-time on a single soak at step=2 vs baseline.

## Out of scope

- `+ 0.3 * cat_value` coefficient (sibling ticket 298).
- `DIST_PENALTY_PER_TILE = 0.005` distance_cost (sibling ticket 299).
- Threat-axis inputs (already ruled out by 285 / 296 / 297).
- Placement decision semantics — argmax-over-additive-sum vs arrest-the-worst-violator (separate, larger ticket 301).
- `HARD_EXCLUDE_MANHATTAN = 3` tightening — separate concern (a tighter exclude with a finer step would conflate two effects).

## Current state

- `CANDIDATE_STEP: i32 = 5` hardcoded at `coordination.rs:1421`.
- Candidate-generation loop at `coordination.rs:1431-1443` (`for cy in (0..map_h).step_by(CANDIDATE_STEP as usize)` × `for cx`).
- Doc comment at `coordination.rs:1418-1420` reads "matching the bucket size of the influence maps". Important nuance: the influence maps' values are already linearly interpolated across their bucket cells (see `PlacementMaps` callers — `maps.fox_scent.get(candidate.x, candidate.y)` reads at arbitrary integer coords, not bucket-aligned), so a finer candidate sampling does not lose information on the influence-map side. The bucket-alignment comment justifies *why 5 was reasonable*, not *why 5 is necessary*.
- Every ward position in 285+296+297's soak `events.jsonl` files sits at multiples of 5. That is the constraint we're testing.

## Approach

1. Promote `CANDIDATE_STEP` to `SimConstants::scoring::ward_placement_candidate_step: i32` with `#[serde(default = default_ward_placement_candidate_step)]` returning `5`. Follow the 296 logistic-params promotion as the pattern.
2. Inline-replace the `const CANDIDATE_STEP: i32 = 5;` site with the constants read; keep the `HARD_EXCLUDE_MANHATTAN` const local for this ticket.
3. `just check` + `just test`. Baseline `just soak 42` should be byte-identical to the pre-promotion run on the same commit (constants-drift clean).
4. Author `docs/balance/hypothesis-300-ward-candidate-step.yaml`: baseline `step=5`, treatment `step=2`, seed 42. Pre-register: "scorer-argmax may shift onto non-multiple-of-5 tiles; `shadow_foxes_avoided_ward_total` Δ direction unknown — this is an exploratory grid-refinement, not a directional prediction." Spatial check pre-registered: at step=2, ward positions in treatment `WardPlaced` events SHOULD include at least one non-multiple-of-5 coordinate (otherwise the grid wasn't the constraint).
5. If seed-42 moves materially (>10% on the characteristic metric), triangulate seeds 99 + 7. If seed-42 holds byte-identical, that's the load-bearing observation — file as the fourth threat-axis-adjacent lever ruled out, and the bayesian update is "the argmax is robust to candidate-grid density too; the lever is `cat_value`, `distance_cost`, or decision semantics — not search resolution."
6. Cost: at step=1 the scorer evaluates ~10800 candidates per ward-placement call. `compute_ward_placement` runs every ~20 ticks per coordinator (gated by `ward_strength_low && thornbriar_available`), so total wall-time impact is bounded. Verify with a `just soak-trace 42` wall-time delta at step=2 vs step=5.

**Structural candidate to surface in iter-1 per CLAUDE.md bugfix discipline:** rather than uniform fine-grained sampling, an **adaptive two-pass candidate generator** — pass 1 at the current coarse `step=5` grid, pass 2 adds candidates at every tile within Manhattan-N of the top-K coarse winners. Achieves intermediate-tile coverage in the high-score regions without paying 25× scoring cost everywhere. Mention as a fork-point for iter-2 if uniform `step=2` shows scoring-cost concern; otherwise it's a follow-on ticket.

## Verification

- `just check` + `just test` green.
- `just soak 42` at default `step=5` byte-identical to pre-ticket baseline (constants-drift clean, ward positions identical).
- `just hypothesize` four-artifact concordance recorded.
- Spatial check: at `step=2`, at least one `WardPlaced` event in the treatment soak has an x or y coordinate not divisible by 5 (proves the grid was the constraint; if all positions remain on multiples of 5, the grid wasn't binding and we've ruled out the fourth lever).
- Five continuity canaries each ≥ 1 across all soaks.
- Wall-time check: scorer cost at step=2 within a tolerable factor (target: full soak wall-time within +25% of baseline).

## Log
- 2026-05-12: opened as lever #3 of four follow-on tickets from 297's iter-2 architectural finding. Most §4.7-clean of the four and cheapest standalone test.
- 2026-05-12: 2026-05-12: hypothesize on seed-42 produced 16/16 WardPlaced events byte-identical to baseline + 0.0% delta on shadow_foxes_avoided_ward_total. Filing as fourth threat-axis-adjacent lever ruled out (joins 285/296/297). Surfaced Path-A vs Path-B finding and just soak vs just sweep non-determinism.
