---
id: 312
title: fox-approach-corridor perception axis for ward placement (301 FO-2)
status: ready
cluster: belief-perception
initiative: []
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [301-ward-placement-decision-semantics.md, 297-fox-patrol-topology-axis.md]
landed-at: null
landed-on: null
---

## Why

301's first-light soak made the substrate gap concrete: `compute_ward_placement`'s score formula (`unaddressed_threat + 0.3 * cat_value − distance_cost + jitter`) has no input that recognizes **topological criticality** — which tiles foxes actually traverse to reach cats. `fox_scent` captures *recent presence* but decays quickly; `corruption` lights up spawn sources but not approach paths; `cat_value` and `-distance_cost` both pull placement toward where cats live rather than toward where threats come from. The result: any selection-rule change (including 301's descending-residual) just moves wards within the cat-cluster, never onto the corridors that foxes use.

This ticket adds the missing perception input: a `FoxApproachCorridorMap` populated by observed `ShadowFox` movement, sampled by the placement scorer as a fourth threat-axis lift. It's the substrate change that makes 301's wiring (or the existing argmax) actually point at the right tiles. Acceptance gate is FO-1 (ticket 311): the isthmus scenario must produce a ward at the corridor when the new axis is activated.

This is the lightweight first cut. FO-4 migrates the same signal into the 258 belief layer once 263–270 establishes the belief-DSE consumer surface.

## Scope

- New resource `FoxApproachCorridorMap` at `src/resources/fox_approach_corridor_map.rs`. Mirror `RecentAmbushMap` shape (bucketed grid, `deposit`, `decay_all`, slow per-day decay, `get(x, y) -> f32`).
- Implement `InfluenceMap` for it in `src/systems/influence_map.rs` (tagged `Sight × Neutral` like `RecentAmbushMap`, or `Sight × Species(ShadowFox)` — pick at implementation time).
- Register in `populate_influence_map_registry` at `src/plugins/simulation.rs`.
- Insert as default resource in `src/plugins/setup.rs` and the scenario harness `src/scenarios/env.rs`.
- Populator hook in the wildlife system chain: when a `ShadowFox` advances position, `corridor.deposit(pos.x, pos.y, w)` at its new tile. Schedule alongside `fox_scent_tick` to avoid a new chain-edge (ticket 061 precedent).
- Wire as a fourth threat-axis lift in `compute_ward_placement` alongside `ambush_lift` / `carcass_lift` / `fox_intercept_lift`. Gated by new `ScoringConstants::ward_fox_approach_corridor_weight: f32`, default `0.0` (dormant at land per 220 / 297 / 301 first-light pattern).
- Flip FO-1's `expected_isthmus_corked` to `true`. The scenario asserts `WardPlaced.location.x ∈ [28, 32]` when the new axis is activated at `weight = 0.3` (test-fixture-level override).
- Unit tests:
  - `corridor_axis_dormant_when_weight_is_zero`: byte-identical to pre-FO-2 at default.
  - `corridor_axis_lifts_score_on_high_traffic_tile`: synthetic placement-maps with a high-corridor tile vs equivalent low-corridor tile; argmax shifts to high-corridor when weight is lifted.
- Three-seed `just hypothesize` four-artifact validation at weight `0.3`: predict modest restoration (or improvement) of `shadow_foxes_avoided_ward_total` on seed-42.

## Out of scope

- Re-balancing `cat_value` / `distance_cost` — FO-3 (ticket TBD, blocked by this one).
- Belief-layer migration via `WitnessableEvent::FoxCrossing` — FO-4 (blocked by this + 263–270 belief-DSE consumers).
- Reactivating descending-residual or intent-weight from 301 — defaults stay where 301 landed them.
- Cat-side path-cost integration (the corridor map should NOT bias cat A* — it's a ward-placement signal only).

## Current state

301 landed dormant. The placement scorer reads four influence maps today (`fox_scent`, `corruption`, `recent_ambush`, `carcass_scent`) plus an inline fox-spawn-vicinity computation. Three of those (ambush, carcass, fox_intercept) compose as logistic lifts on a shared shape; this ticket adds the fourth in the same pattern.

`RecentAmbushMap` (ticket 219) is the closest existing precedent: per-tick deposit by an event handler, slow global decay, sampled by the placement scorer. The corridor map differs by trigger — it deposits on **movement** (every passing fox) rather than **event** (only ambushes), so the signal density is higher and the decay rate needs to be slower-still to avoid washing out.

## Approach

1. **Resource shape.** `pub struct FoxApproachCorridorMap { marks: Vec<f32>, grid_w: usize, grid_h: usize, bucket_size: i32 }` exactly mirroring `RecentAmbushMap`. Default `120×90` map with 5-tile buckets.

2. **Populator.** In `src/systems/wildlife.rs`, find the fox-movement step (likely `fox_movement` or `wildlife_ai`'s ShadowFox branch). After the fox's new position is committed, call `corridor.deposit(pos.x, pos.y, deposit_per_step)`. New constant `corridor_deposit_per_step: f32` on `WildlifeConstants`, default ~`0.05` (saturates over ~20 visits, matches `cat_patrol_deterrent_deposit_per_tick` shape). Skip the deposit when the fox is on its starting tile or in a Patrol-idle state (avoid double-counting stationary foxes).

3. **Decay.** New per-tick decay system `decay_fox_approach_corridor_map` scheduled inside an existing wildlife sub-chain (NOT as a new top-level edge — schedule-edge perturbation risk per ticket 061). Constant `corridor_decay_per_day: RatePerDay`, default `0.2/day` (slower than fox_scent's 0.1/day because corridors are stable terrain features, not transient marks).

4. **Consumer.** Inside `compute_ward_placement`'s scoring loop:
   ```rust
   let corridor_lift = if w_corridor > 0.0 {
       w_corridor * logistic_threat_lift(
           maps.corridor.get(candidate.x, candidate.y),
           curve_k,
           curve_m,
       )
   } else {
       0.0
   };
   let threat = (fox_scent.max(corruption) + ambush_lift + carcass_lift + fox_intercept_lift + corridor_lift).min(1.0);
   ```
   The `PlacementMaps` struct grows a 7th field `corridor: &FoxApproachCorridorMap`; the `WardPlacementSignals` SystemParam grows the matching `Res<>`.

5. **Composition concern (decide at implementation time).** Adding a fifth additive lift to the saturating threat sum risks the same rank-preservation problem 297 iter-2 documented. Two design options, pick one:
   - **(a)** Compose corridor as a **separate multiplicative term** outside the saturating sum: `score = unaddressed_threat * (1 + w_corridor * L(corridor)) + 0.3 * cat_value − distance_cost + jitter`. Lets the corridor lift the score *above* 1.0 in the high-threat band, breaking the saturation rank-preservation.
   - **(b)** Compose corridor as **inside the sum but with higher logistic steepness** (e.g., midpoint=0.2 instead of 0.5). Easier to land; doesn't escape the saturation cap.
   
   Lead with (a). It directly addresses the 297 iter-2 architectural finding instead of stacking against it.

6. **Activation gating.** Default `ward_fox_approach_corridor_weight: 0.0`. The first-light commit lifts to `0.3` and runs the FO-1 scenario + three-seed soak.

## Verification

- `just check` + `just test` green; new unit tests pass.
- FO-1 isthmus scenario passes the isthmus-corked assertion at `weight = 0.3` (scenario flips `expected_isthmus_corked: true`).
- Three-seed `just hypothesize` four-artifact (seeds 42 / 99 / 7):
  - `wards_placed_total` within ±15%.
  - `shadow_foxes_avoided_ward_total` direction match (lift, ≥ +20% from the dormancy baseline on seed-42).
  - All five continuity canaries ≥ 1.
  - `deaths_by_cause.Starvation == 0`; `deaths_by_cause.ShadowFoxAmbush ≤ 10`.
- Default-flag byte-identity soak: `just soak 42` at `weight = 0.0` produces byte-identical `WardPlaced` set vs the post-FO-2 commit's parent.
- Substrate-stub allowlist clean (new InfluenceMap impl ships with registry call in same commit).
- Update `docs/balance/301-ward-placement-decision-semantics.md` with an iter-2 follow-on entry naming this ticket's outcome.

## Log

- 2026-05-13: opened from 301's findings-only landing. Blocked by 311 (FO-1 scenario).
