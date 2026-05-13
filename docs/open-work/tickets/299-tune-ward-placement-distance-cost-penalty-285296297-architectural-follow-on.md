---
id: 299
title: tune ward placement distance_cost penalty (285+296+297 architectural follow-on)
status: ready
cluster: buildings-zones
initiative: []
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [284-ward-anchor-tuning.md, 297-fox-patrol-topology-axis.md]
landed-at: null
landed-on: null
---

## Why

Three independent threat-axis levers have now been ruled out as ways to move `shadow_foxes_avoided_ward_total`: 285 (anchor-weight magnitude), 296 (Logistic curve shape), and 297 (adding an orthogonal threat-axis input). All three produced byte-identical placement across seeds 42 / 99 / 7. The architectural finding from `docs/balance/297-fox-patrol-topology-axis.md` iter-2: once any single threat-side input saturates on a sufficient number of tiles, the threat-axis composition is rank-preserving and the placement argmax is decided by the **non-threat-axis score terms**.

297 iter-2 names exactly four non-threat-axis structural levers for follow-on work:
1. `+ 0.3 * cat_value` coefficient — ticket 298.
2. **`distance_cost` term** ← *this ticket*.
3. Candidate-generation step — ticket 300.
4. Placement decision semantics — argmax-over-additive-sum (separate larger ticket 301).

This ticket addresses lever #2. The current penalty (`coordination.rs:1428`, `DIST_PENALTY_PER_TILE = 0.005`) was picked dimensionlessly against the `[0, 1]` threat axis ("100-tile detour costs 0.5 score") without empirical sweep. Per 297's iter-2 framing: tightening it would prevent placement from reaching corruption-zone tiles outside the anchor's local Manhattan ring; loosening it would let placement reach more distant high-threat tiles. Either direction is a plausible mover of the avoided-ward counter, and neither has been measured.

## Scope

- Promote `DIST_PENALTY_PER_TILE` from a file-local `const` to `SimConstants` as `ward_placement_distance_penalty_per_tile`, with `#[serde(default = ...)]` defaulting to `0.005` to preserve current behavior byte-for-byte.
- Update the use-site at `coordination.rs:1494` to read from constants.
- Add a regression test verifying that at the default value, placement output is byte-identical to pre-extraction (seed-42 anchor).
- Run a `just hypothesize` four-artifact sweep across candidate values `{0.002, 0.005 baseline, 0.01, 0.02}` on seed-42 first, then triangulate seeds 99 and 7.
- Write `docs/balance/299-ward-placement-distance-penalty.md`.

## Out of scope

- `+ 0.3 * cat_value` coefficient — sibling ticket 298 (lever #1 from 297 iter-2).
- Candidate-generation step — sibling ticket 300 (lever #3 from 297 iter-2).
- Threat-axis inputs — already ruled out by 285 / 296 / 297.
- Placement decision semantics — separate larger ticket 301 (lever #4 from 297 iter-2).

## Current state

- `DIST_PENALTY_PER_TILE = 0.005` is a file-local `const` at `src/systems/coordination.rs:1428`.
- Doc comment at `coordination.rs:1422-1427` justifies the value dimensionlessly against the `[0, 1]` threat axis; explicitly says "no balance constant needed." That dimensional-analysis reasoning has not been validated by sweep.
- Use-site is the score formula at `coordination.rs:1494`: `score = unaddressed_threat + 0.3 * cat_value - distance_cost + jitter`.
- Anchor is the structure-cluster centroid (`coordination.rs:1398-1406`). `placement_radius` is *not* used today — every coarse-grid (step 5) tile across the whole map is a candidate, hard-excluding only Manhattan-3 around existing wards.
- `distance_cost` scales linearly with Manhattan distance from anchor.

## Approach

1. Add `ward_placement_distance_penalty_per_tile: f32` to `SimConstants::scoring` with `#[serde(default = "default_ward_placement_distance_penalty_per_tile")]` returning `0.005`. Doc-comment the field with the existing `:1422-1427` rationale plus a "see 297 iter-2 architectural finding" pointer.
2. Replace the `const` at `coordination.rs:1428` with a read from `constants.scoring.ward_placement_distance_penalty_per_tile` (clamp `>= 0.0` defensively).
3. Add a regression test: at the default, `compute_ward_placement` returns identical positions to a captured pre-extraction baseline on seed-42 (mirrors ticket-296's `logistic_threat_lift_at_defaults_matches_pre_296_curve` regression guard).
4. Author `hypothesize-299-ward-distance-penalty.yaml` sweeping `{0.002, 0.005, 0.01, 0.02}` on seed-42; triangulate against seed-99 and seed-7 per 285's discipline.
5. **Pre-register predictions.** Tightening (`0.01`, `0.02`) → placement localizes near the anchor, reducing reach into outer corruption zones; `shadow_foxes_avoided_ward_total` predicted *down* on seeds where the outer-ring tiles were carrying the counter. Loosening (`0.002`) → placement scatters toward distant high-threat tiles; counter predicted *up*. The metric-prediction band for this knob is **wider** than for `cat_value` because distance touches both ends of the formula — anchor-proximity (cat-side) and reach-to-corruption (threat-side) — instead of just one.

**Structural candidate to mention per CLAUDE.md bugfix discipline:** the load-bearing primitive is Manhattan distance from a centroid. A more honest formulation would be **travel-cost on pathable terrain** (the cat-pathing infrastructure already exists in the codebase). That would penalize unreachable tiles — water, impassable corruption — instead of treating them as cheap if Manhattan-close. Out of scope here; flagged for the larger placement-semantics ticket (lever #4 / ticket 301).

## Verification

- `just check` + `just test` green; regression test asserts byte-identity at default.
- `just hypothesize` concordance call across seeds 42 / 99 / 7.
- Spatial check: placement positions shift in the predicted direction (tightening pulls wards toward the structure centroid; loosening reaches farther from it).
- Five continuity canaries each `>= 1` on the treatment soaks; `deaths_by_cause.Starvation == 0`; `never_fired_expected_positives == []`.
- Constants-drift-vs-baseline clean; `just verdict` exits `pass` on all three treatment seeds.

## Log
- 2026-05-12: opened as lever #2 of four follow-on tickets from 297's iter-2 architectural finding.
