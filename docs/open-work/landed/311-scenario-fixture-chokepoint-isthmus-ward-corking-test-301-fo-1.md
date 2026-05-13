---
id: 311
title: scenario fixture — chokepoint isthmus ward-corking test (301 FO-1)
status: done
cluster: tooling-diagnostics-ui
initiative: []
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [301-ward-placement-decision-semantics.md]
landed-at: a04b27e44047
landed-on: 2026-05-13
---

## Why

Ticket 301 (landed substrate-no-op) lifted a structural lever on ward-placement selection (descending-residual + intent map) but its first-light soak was anti-concordant on `shadow_foxes_avoided_ward_total` (`-100%` on seed-42). Root cause: the placement scorer's input formula (`unaddressed_threat + 0.3 * cat_value − distance_cost + jitter`) has no input that recognizes **topological criticality** — chokepoints, fox-approach corridors, traversal bottlenecks. The selection rule cannot bias placement toward the right tiles when the score function doesn't see them.

This ticket lands the test fixture that any follow-on substrate change (FO-2: corridor perception axis) must satisfy. A narrow-isthmus map exercises the "cork the chokepoint, don't paint the landmass" behavior the user named in 301's planning session: cats on one landmass, foxes on the other, connected by a single corridor. The desired placement algorithm should pick the corridor; the current algorithm picks an interior tile near cats.

## Scope

- New scenario module `src/scenarios/chokepoint_defense_isthmus.rs`.
- Register in `src/scenarios/mod.rs::ALL`.
- `expected_features: &["CropHarvested", "GatherHerbCompleted"]` — gates the farming + wild-pickup chains the user noted as underexercised by current canaries. **WardPlaced is exercised but not gated at FO-1.** The substrate stalls HerbcraftSetWard at L3 election under a work-pinned profile within an affordable tick budget; the reference `ward_placement.rs` fixture has the same limitation and also opts out of `WardPlaced` gating. FO-2's corridor perception axis is expected to lift the L2 score enough to fire reliably, at which point this scenario's `expected_features` gains `"WardPlaced"`.
- Scenario lands GREEN under existing defaults.
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

Concrete fixture spec (as shipped):

- 60×40 `TileMap` via `ScenarioWorldConfig { width: 60, height: 40, colony_center: Position::new(45, 20) }`.
- `Terrain::Water` everywhere except:
  - **West landmass**: x ∈ [5, 25], y ∈ [10, 30].
  - **East landmass**: x ∈ [35, 55], y ∈ [10, 30].
  - **Isthmus**: a 2-tile-wide corridor at x ∈ [27, 33], y ∈ [19, 21] (centered on (30, 20)).
- East-landmass population — **one work-pinned cat** (Talon at (40, 20)):
  - Pattern mirrored from `farm_herb_demand.rs` and `ward_placement.rs`. An earlier 3-cat draft L3-elected `Socialize` continuously and never reached the work DSEs.
  - Personality: diligence/patience/spirituality/compassion/tradition high; curiosity/boldness/playfulness/sociability pinned low so non-work DSEs don't crowd Farm/HerbcraftGather/HerbcraftSetWard.
  - `magic_affinity = 0.6` (mirrors `ward_placement.rs` for ward scoring lift).
  - Needs: hunger=1.0 sated, energy=0.9, purpose=0.3 (unmet purpose motivates work).
  - Markers: `Adult` + `CanForage`; `CanWard` authored automatically by `update_capability_markers` once `HasWardHerbs` lands.
  - `Skills.foraging = 1.0` post-spawn so tend cycles complete in ~10 ticks (matches `farm_herb_demand.rs`).
  - 1 thornbriar pre-loaded in inventory via `give_herbs`.
- A pre-mature Thornbriar **garden** on the east landmass at (42, 22) with `CropState { growth: 0.95, crop_kind: CropKind::Thornbriar }`. Authors `HasGarden` colony marker on tick 1; `FarmDse` becomes eligible; tend cycle finishes in ~10 ticks and `Feature::CropHarvested` fires.
- Two **wild thornbriar herb entities** at (48, 19) and (52, 24): `Herb { kind: Thornbriar, growth_stage: Blossom, magical: false, twisted: false } + Seasonal { available: year-round } + Harvestable`. `HerbcraftGatherDse` picks them up; `Feature::GatherHerbCompleted` fires.
- A small **corruption gradient** near the isthmus's east mouth (via `mark_tile_corrupted`) so `is_ward_strength_low` registers as colony-priority and `HerbcraftSetWard`'s L2 score has a meaningful target. Mirrors `ward_placement.rs`'s setup.
- West-landmass population: one `WildAnimal { species: ShadowFox }` at (15, 20) with `WildlifeAiState::Patrolling { dx: 1, dy: 0 }` heading east toward the isthmus.
- `default_focal = "Talon"`, `default_ticks = 250` — covers the full Farm→Tend→Harvest cycle + wild-herb gather. The reduced gate (no WardPlaced) means the cheaper 60–80 tick budget used by sibling scenarios doesn't apply here.

Pattern mirror: `src/scenarios/farm_herb_demand.rs` (work-pinned single-cat profile + Skills.foraging override), `src/scenarios/ward_placement.rs` (spirituality + magic_affinity + corruption gradient), `src/scenarios/fox_ward_only_avoidance.rs` (ShadowFox spawn shape).

`expected_isthmus_corked: bool` is a new local boolean on this scenario only — not a Scenario-struct field. At FO-1 land it's `false` and the scenario asserts nothing about location. At FO-2 land the field flips to `true`, `WardPlaced` lands in `expected_features`, and the scenario asserts `WardPlaced.location.x ∈ [28, 32]`. This sequencing keeps FO-1 a pure-fixture PR with no behavioral commitment.

## Verification

- `cargo test --test scenarios` — all 4 tests pass, including `declared_expected_features_all_fire` with the reduced `["CropHarvested", "GatherHerbCompleted"]` gate.
- `just check` clean (fmt/clippy/step-resolver/time-units/iaus/substrate-stubs/items-are-real/influence-map-registry).
- `just scenario list` — `chokepoint_defense_isthmus` appears with `default_focal=Talon, default_ticks=250`.
- `just scenario chokepoint_defense_isthmus --focal Talon --ticks 250` smoke: per-tick winners include `Farm` (4×) and `HerbcraftGather` (2×) within the budget. `herbcraft_ward` shows `!!` (ineligible at L3) in the L2 dump — the substrate stall FO-2 will address.

## Log

- 2026-05-13: opened from 301's findings-only landing. Blocks FO-2 (corridor perception axis).
- 2026-05-13: landed. User-directed scope expansion at plan time ("also test ward herb farming here, since afaik we still have no farming canaries firing") restructured the fixture from a 3-cat ward-placement scenario to a single work-pinned cat exercising Farm + HerbcraftGather + HerbcraftSetWard end-to-end. The first two gate `expected_features`; the third is exercised but not gated at FO-1 (reference `ward_placement.rs` has the same opt-out for the same reason — HerbcraftSetWard stalls at L3 election under work-pinned profiles in affordable tick budgets). Documented in scenario module rustdoc as FO-2's acceptance surface.
