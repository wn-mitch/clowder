---
id: 297
title: ward placement needs fox-patrol-topology perception axis (285 follow-on)
status: done
cluster: ai-substrate
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [284-ward-anchor-tuning.md]
landed-at: 6756dbe465ec
landed-on: 2026-05-12
---

## Why
285's three-seed spatial scan revealed that ward placement and fox patrol routes have a structural perception gap. The `compute_ward_placement` threat axis reads `max(fox_scent, corruption) + w_ambush·L(recent_ambush) + w_carcass·L(carcass_scent)` — all four signals are *consequences of past fox activity near cats*, not predictors of *where future fox patrols will enter the colony*. On seed-42 the cat-side ambush memory pulls wards to (29-42, 10-36) while fox spawn sites (corruption=1.0 ruins) sit at (24-114, 42-83) in the south/east; only 2-out-of-30 spawned foxes ever traverse the placed wards. On seed-7 the geometry happens to overlap and the avoidance counter is at 78. The substrate is correct-but-insufficient: ambush memory says *where cats got hurt*, not *where foxes come from*. A complementary axis that encodes fox-spawn topology and patrol-route entry geometry — call it `ward_fox_intercept_anchor_weight` against a `FoxSpawnVicinityMap` or similar — would let `compute_ward_placement` reason about *interception likelihood*, not just ambush recency. This is the orthogonal-axis discipline (CLAUDE.md Design pillar 3): add a new axis that perceives a distinct situation, don't amplify what's already there.

## Scope
- Design the new perception axis. Candidate substrate: a per-tile `FoxSpawnVicinityMap` that decays from corruption-tile centroids, OR direct read of the existing `CorruptionLens` / fox_scent maps with a placement-side reweighting that *inverts* the usual cat-presence pull (wards should land *between* corruption and cats, not *at* cats).
- Implement the new axis + corresponding `SimConstants` field + Logistic lift in `compute_ward_placement`.
- Write a balance writeup `docs/balance/297-fox-patrol-topology-axis.md` running the four-artifact methodology across seeds 42, 99, 7. Predicted metric: `shadow_foxes_avoided_ward_total` should lift materially on the *low-counter seeds* (42) and hold or lift on the high-counter seeds (7).

## Out of scope
- Re-tuning the existing `ward_ambush_anchor_weight` and `ward_recency_anchor_weight` magnitudes — 285 is conclusive that magnitude is inert. The new axis composes alongside the existing two; weights for all three may need joint re-tuning AFTER 296's curve-shape work.
- Touching the Logistic curve shape — that's 296's surface.
- Reactive ward migration / removal.
- Cleansing / banishment knobs.

## Current state
220 landed the substrate plumbing (ambush + carcass anchors). 284 activated the weights `(0.5, 0.3)` first-light. 285 ran the four-artifact magnitude sweep across three seeds and surfaced the perception gap as the deepest architectural finding (see `docs/balance/284-ward-anchor-tuning.md` iter-2 §Spatial topology check).

## Approach
This is a §4 marker / §6 target-taking design, not a parameter tune. Walk `docs/systems/ai-substrate-refactor.md` §4.7 substrate-vs-search-state before designing the axis — `compute_ward_placement` is a per-tile scoring function, so the new axis must be a perceivable per-tile signal (substrate), not a planner-side search heuristic. Then:
1. Sketch the substrate source. If `CorruptionLens` already exposes per-tile corruption strength, the cheap version is a `Logistic` lift on `corruption.distance_kernel(...)` — a per-tile "this tile is near a fox spawn" signal. If a new map is needed, follow the InfluenceMap registry pattern (`scripts/check_influence_map_registry.sh`).
2. Add reader (`compute_ward_placement` consumption) + writer (the new map populator system) in the same commit per the substrate-stub doctrine.
3. Land the axis dormant at `0.0`; activate first-light per the `feedback_dormant_substrate_activation_soak_first` pattern.

## Verification
- All hard survival gates and continuity canaries hold across the four-artifact sweep.
- `shadow_foxes_avoided_ward_total` lifts on seed-42 (currently 2) toward seed-99's level (20) or higher — the metric should converge across seeds as the substrate stops being topology-sensitive.
- Per-seed spatial scan after the lift: wards visibly migrate toward the *boundary* between fox-spawn sites and cat zones, not deeper into the cat zone.

## Log
- 2026-05-12: opened as the perception-axis follow-on to 285. The orthogonal-axis discipline (CLAUDE.md Design pillar 3) drives the framing — see also memory `feedback_single_axis_perception_scalars` and `project_l3_patrol_absorption_cascade` for prior precedents where adding orthogonal axes fixed substrate-vs-outcome gaps.
- 2026-05-12: First-light succeeds (layer fires per unit tests; continuity canaries hold per clean hypothesize-comparison; small positive continuity drift across all three seeds). Three-seed four-artifact sweep at (0.0→0.5) produced byte-identical placement on seeds 42/99/7 — joins 285 (magnitude inert) and 296 (curve shape inert) as the third independent threat-axis lever ruled out. Architectural conclusion sharpens: placement argmax is determined by non-threat terms (cat_value/distance_cost/jitter) once threat saturates. Future placement-metric movement requires structural change at a layer outside the threat-axis-additive composition (cat_value coefficient, distance_cost, candidate-generation step, or decision semantics).
