---
id: 311
title: scenario fixture — chokepoint isthmus ward-corking test (301 FO-1)
status: ready
cluster: tooling-diagnostics-ui
initiative: []
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [301-ward-placement-decision-semantics.md]
landed-at: null
landed-on: null
---

## Why

Ticket 301 (landed substrate-no-op) lifted a structural lever on ward-placement selection (descending-residual + intent map) but its first-light soak was anti-concordant on `shadow_foxes_avoided_ward_total` (`-100%` on seed-42). Root cause: the placement scorer's input formula (`unaddressed_threat + 0.3 * cat_value − distance_cost + jitter`) has no input that recognizes **topological criticality** — chokepoints, fox-approach corridors, traversal bottlenecks. The selection rule cannot bias placement toward the right tiles when the score function doesn't see them.

This ticket lands the test fixture that any follow-on substrate change (FO-2: corridor perception axis) must satisfy. A narrow-isthmus map exercises the "cork the chokepoint, don't paint the landmass" behavior the user named in 301's planning session: cats on one landmass, foxes on the other, connected by a single corridor. The desired placement algorithm should pick the corridor; the current algorithm picks an interior tile near cats.

## Scope

- New scenario module `src/scenarios/chokepoint_defense_isthmus.rs`.
- Register in `src/scenarios/mod.rs::ALL`.
- `expected_features: &["WardPlaced"]` so the canonical features test gates that the scenario *does* produce a ward.
- Scenario lands GREEN under existing defaults (selection still happens, just at the wrong tile per the architectural finding).
- Behavioral assertion ("`WardPlaced.location.x` within ±2 of the isthmus center x=30") is **not** added at FO-1 land. That's FO-2's acceptance gate. The scenario module gains an `expected_isthmus_corked: bool` flag set to `false` at FO-1 land, flipped to `true` by FO-2's PR.

## Out of scope

- The corridor perception axis itself — FO-2 (ticket TBD, blocked by this one).
- Cat-value / distance-cost re-tuning — FO-3 (ticket TBD, blocked by FO-2).
- Belief-layer migration of corridor signal — FO-4 (longer horizon, blocked by FO-2 + tickets 263–270).
- Behavioral isthmus-corked assertion (added by FO-2).

## Current state

- 301 lands the placement-substrate wiring dormant: `WardPlacementSemantics`, `WardIntentMap`, `via_directive` on `WardPlaced`, conditional 4th axis on `HerbcraftWardDse`. All defaults preserve byte-identity to pre-301.
- The substrate is ready; the perception layer that would make it useful is not.
- `src/scenarios/fox_ward_only_avoidance.rs` is the closest existing fixture in shape (single-fox + cat + ward expectation). Pattern to mirror.
- `src/scenarios/env.rs` provides `ScenarioWorldConfig` (map dimensions, colony-center) and `init_scenario_world_with()`. Terrain customization happens by direct `world.resource_mut::<TileMap>()` writes in the scenario's setup closure.

## Approach

Concrete fixture spec:

- 60×40 `TileMap` via `ScenarioWorldConfig { width: 60, height: 40, colony_center: Position::new(45, 20) }`.
- Mark most tiles `Terrain::Water` (impassable in pathfinding) via per-tile writes in setup. Two grass landmasses:
  - **West landmass**: x ∈ [5, 25], y ∈ [10, 30].
  - **East landmass**: x ∈ [35, 55], y ∈ [10, 30].
  - **Isthmus**: a 2-tile-wide corridor at x ∈ [27, 33], y ∈ [19, 21] (2 tiles wide × 7 tiles long, centered on (30, 20)).
- East-landmass population (cats):
  - 3 cats via `spawn_cat(world, CatPreset::adult(name, pos))` at (40, 18), (45, 22), (50, 20).
  - Personalities: high `diligence`, moderate `boldness` (warding-capable cats).
  - Markers: `CanWard`, `WardStrengthLow`, `HasWardHerbs` — eligibility-complete for `HerbcraftWardDse`.
  - Thornbriar inventory pre-loaded (1–2 herbs each via `Inventory` insert).
- West-landmass population (foxes):
  - One `WildAnimal { species: ShadowFox }` at (15, 20) with `WildlifeAiState::Patrolling { dx: 1, dy: 0 }` heading east toward the isthmus.
  - Standard `Health`, `SensorySpecies`, `SensorySignature::WILDLIFE` per `fox_cat_scent_avoidance.rs` pattern.
- A small thornbriar patch on the east landmass (single `Herb` entity with `Harvestable`) so herb supply is not the gating factor.
- `default_focal = Some("Talon")` (one of the warding cats) for trace-side observation if the user runs `just scenario chokepoint_defense_isthmus --focal Talon`.
- `default_ticks = 60` — long enough for the fox to traverse half the map and a cat to plant at least one ward; short enough that `cargo test scenario_feature_assertions` stays cheap.

Pattern mirror: `src/scenarios/fox_ward_only_avoidance.rs` for the spawn / setup shape; `src/scenarios/hunt_acquisition.rs` for the `CatPreset` builder usage.

`expected_isthmus_corked: bool` is a new local boolean on this scenario only — not a Scenario-struct field. At FO-1 land it's `false` and the scenario asserts nothing about location. At FO-2 land the field flips to `true` and the scenario asserts `WardPlaced.location.x ∈ [28, 32]`. This sequencing keeps FO-1 a pure-fixture PR with no behavioral commitment.

## Verification

- `cargo test scenario_feature_assertions` — passes with `expected_features: &["WardPlaced"]`. The scenario produces at least one ward in 60 ticks.
- `just scenario chokepoint_defense_isthmus --focal Talon --ticks 20` — manual smoke: per-tick winning DSE table shows `HerbcraftSetWard` eventually winning for one of the cats; `WardPlaced` event lands somewhere on the east landmass (NOT corked at the isthmus — that's the bug FO-2 fixes).
- `just check` clean.
- Scenario appears in `just scenario list`.

## Log

- 2026-05-13: opened from 301's findings-only landing. Blocks FO-2 (corridor perception axis).
