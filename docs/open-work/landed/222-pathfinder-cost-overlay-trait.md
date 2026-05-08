---
id: 222
title: pathfinder cost-overlay trait
status: done
cluster: pathfinder-risk-awareness
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: dfe9a7ec6aa1
landed-on: 2026-05-07
---

## Why
The pathfinder (`src/ai/pathfinding.rs:67-138`) is a standard
8-directional A* over `&TileMap` whose only edge cost is
`terrain.movement_cost()` (Grass=1, Forest=3, Rock=4). It is
**scent-blind and influence-map-blind**. The 14× influence maps already
in the sim (`FoxScentMap`, `CarcassScentMap`, `CorruptionLens`,
`WardCoverageMap`, `CatPresenceMap`, etc., registered via
`populate_influence_map_registry` in `src/plugins/simulation.rs:131-160`)
inform DSE *scoring* but cannot influence *routing*. This is the
substrate gap surfaced by ticket 214's investigation: 209 wired a
Patrol L2 cost-axis on `fox_scent_level`, but a DSE-score axis runs
*after* the route is chosen — it can damp "should I patrol?" but it
can't make the cat route around fox territory. That requires risk-aware
A*, which the pathfinder cannot express today.

This ticket lands the substrate refactor only — the trait, the
pathfinder API change, and the call-site updates with empty overlay
slices. Behavior-preserving by construction. Subsequent tickets in the
`pathfinder-risk-awareness` cluster (223, 224) wire specific overlays
and personality conditioning.

## Scope
- New trait `TileCostOverlay` in `src/ai/pathfinding.rs` (or sibling
  module): `fn cost_at(&self, pos: Position) -> u32`.
- Refactor `find_path(from, to, map, overlays: &[&dyn TileCostOverlay])`
  — overlay costs add to per-edge `tentative_g`; heuristic stays
  Chebyshev (admissibility preserved iff overlay costs ≥ 0).
- Refactor `step_toward(from, to, map, overlays)` — greedy variant
  evaluates same overlay cost per candidate.
- Update **all 15+ call sites** to pass `&[]`. Call sites:
  - `src/steps/disposition/patrol_to.rs:50`
  - `src/steps/magic/apply_remedy.rs:60`
  - `src/steps/building/tend.rs:71`, `repair.rs:65`,
    `pickup_material.rs:91`, `move_to.rs:54`, `construct.rs:78`
  - `src/steps/fox/mod.rs:30,76`
  - `src/systems/disposition.rs:1703,3694,3703,3729`
  - `src/systems/goap.rs:4158,4421,4650,4751`
- Update existing pathfinding tests to compile with new signature.
- Add unit test: `find_path` with a synthetic high-cost overlay forces
  a documented detour; without it, takes the direct route.

## Out of scope
- Any `impl TileCostOverlay for <FieldType>` — that is ticket 223.
- Any cat-side overlay set beyond `&[]` — also 223.
- Personality conditioning / per-cat weights — ticket 224.
- Retiring `FoxTerritorySuppression` — 223 (the modifier survives this
  ticket because no overlays are wired yet).

## Current state
- Pathfinder is at `src/ai/pathfinding.rs` (486 lines, A* + greedy
  step_toward + find_free_adjacent).
- Influence-map registration precedent at
  `src/plugins/simulation.rs:131-160` (the trait shape parallels
  `InfluenceMap`).
- Ticket 214 is parked pending this cluster; 215 unrelated.
- Ticket A in the cluster — A→B→C blocked-by chain.

## Approach
1. Define trait in `src/ai/pathfinding.rs`:
   ```rust
   pub trait TileCostOverlay {
       fn cost_at(&self, pos: Position) -> u32;
   }
   ```
   Document admissibility constraint (non-negative; Chebyshev
   heuristic stays admissible for any non-negative additive cost).
2. Refactor `find_path` signature; inside the neighbor loop:
   ```rust
   let overlay_cost: u32 = overlays.iter().map(|o| o.cost_at(neighbor)).sum();
   let tentative_g = current_g + terrain.movement_cost() + overlay_cost;
   ```
3. Refactor `step_toward` similarly — accumulate overlay cost per
   candidate, pick lowest. (The current greedy variant doesn't track
   cost — the new version does, which is a behavior change at any
   non-empty overlay slice but a no-op at `&[]`. Document in the
   commit / Log.)
4. Walk every call site; pass `&[]`. The empty-slice path through
   `iter().sum()` collapses to 0, so behavior is preserved.
5. Tests:
   - Existing tests adjust the call signature; pass.
   - New: synthetic overlay with `cost_at` returning a high value on
     a tile blocking the direct route; verify the path detours.
   - New: empty overlay slice produces same path as terrain-only
     legacy.

## Verification
- `just check && just test` — every existing test passes; signature
  change propagated cleanly.
- New `find_path_overlay_forces_detour` and
  `find_path_empty_overlay_matches_legacy` unit tests pass.
- `just soak-trace 42 Wren` produces the **same** end-of-run position
  trail as pre-refactor (modulo any deterministic-ordering perturbations
  introduced by the new struct slots — none expected if `&[]`
  short-circuits cleanly). Commit hash check + footer-vs-baseline drift
  via `just verdict logs/tuned-42` should be at-most ±0.5% on every
  metric, and ideally zero.
- A* admissibility is provable by inspection — non-negative additive
  costs preserve `f(n) ≤ g(n) + h(n)` when h is Chebyshev. No formal
  proof needed; document the property in the trait rustdoc.

## Log
- 2026-05-07: opened from work-214 investigation. The L2 axis 209
  wired on Patrol's CompensatedProduct double-prices `fox_scent_level`
  with `FoxTerritorySuppression`; the destination-aware fix lives
  at the pathfinder layer, not the DSE layer.
