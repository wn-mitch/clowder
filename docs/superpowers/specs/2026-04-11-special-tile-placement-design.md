# Special Tile Placement in World Generation

## Problem

Four special terrain types (`AncientRuin`, `FairyRing`, `StandingStone`, `DeepPool`) exist in the `Terrain` enum with full downstream wiring — corruption seeding in `initialize_tile_magic`, corruption spread, tile effects, shadow fox spawning, ward decay, personal corruption effects, Dreamroot herb spawning, SpiritCommunion actions, and `on_special_terrain` scoring. But `generate_terrain()` never places them, so the entire corruption pipeline (5 systems) and magic-site gameplay are inert. This is the single biggest dead cluster in the simulation.

## Solution

Add a `place_special_tiles` step to world generation that stamps all 4 special tile types onto the Perlin-generated terrain map using Poisson-disk placement with terrain affinity constraints.

## Pipeline Reorder

`find_colony_site` is read-only on the map, so it moves before special tile placement to supply the colony-distance constraint for AncientRuin.

```
1. generate_terrain(120, 90, rng)
2. find_colony_site(&map, rng)                                           ← moved up
3. place_special_tiles(&mut map, colony_site, rng, &constants.world_gen) ← NEW
4. initialize_tile_magic(&mut map, rng)                                  ← finds placed tiles
5. spawn_starting_buildings(world, colony_site, &mut map)
```

Three callsites: `src/plugins/setup.rs`, `src/main.rs`, `tests/integration.rs`.

## WorldGenConstants

New struct in `src/resources/sim_constants.rs`, nested in `SimConstants` with `#[serde(default)]`.

| Field | Type | Default | Purpose |
|---|---|---|---|
| `ancient_ruin_count` | `usize` | 3 | Target ruins per map |
| `fairy_ring_count` | `usize` | 2 | Dreamroot/SpiritCommunion sites |
| `standing_stone_count` | `usize` | 3 | Mystery/ritual sites |
| `deep_pool_count` | `usize` | 2 | Near-water narrative sites |
| `special_min_spacing` | `i32` | 15 | Min manhattan distance between any two special anchors |
| `corruption_colony_min_distance` | `i32` | 30 | AncientRuin ↔ colony minimum |
| `edge_margin` | `i32` | 10 | Keep sites away from map edges |
| `max_placement_attempts` | `usize` | 500 | Max candidates to evaluate per type after shuffle |

## Terrain Affinity

Each type can only replace specific base terrains, matching the world's naturalistic feel.

| Type | Allowed Base Terrain | Extra Rule |
|---|---|---|
| AncientRuin | Grass, Sand | — |
| FairyRing | Grass, LightForest | — |
| StandingStone | Grass, Rock, Sand | — |
| DeepPool | Grass, Mud | ≥1 Water in 4-neighbors |

## Footprint Shapes

| Type | Shape | Tiles | Notes |
|---|---|---|---|
| AncientRuin | 2×2 solid | 4 | All tiles become AncientRuin |
| FairyRing | 3×3 hollow ring | 8 | Center tile unchanged |
| StandingStone | 1×1 | 1 | — |
| DeepPool | 1×1 | 1 | — |

FairyRing layout (F = FairyRing, `.` = original terrain):
```
F F F
F . F
F F F
```

## Placement Algorithm

New module: `src/world_gen/special_tiles.rs`.

Adapted from the Poisson-disk pattern in `src/world_gen/prey_ecosystem.rs:17-38`.

```
fn place_special_tiles(map, colony_site, rng, constants) -> PlacementReport
```

1. Maintain a shared `Vec<Position>` of all placed special tile anchors.
2. Process types in constraint-tightness order: AncientRuin → FairyRing → StandingStone → DeepPool.
3. For each type:
   a. Collect candidate tiles: in bounds for full footprint, all footprint tiles match terrain affinity, ≥ `edge_margin` from map edges.
   b. Shuffle candidates (deterministic via `rng`).
   c. Accept if: manhattan distance from anchor to every placed anchor ≥ `special_min_spacing`, and (for AncientRuin) manhattan distance to `colony_site` ≥ `corruption_colony_min_distance`.
   d. Stamp footprint, record anchor.
   e. Stop at target count or candidate exhaustion.
4. Log placement counts via `eprintln!` (matches prey_ecosystem.rs pattern).

For multi-tile footprints, distance is measured from anchor (top-left). The footprints are small (max 3×3) relative to the 15-tile spacing, so anchor-based distance is sufficient.

## Cascade Activation

Once AncientRuin tiles exist with corruption 0.5–0.8 (seeded by `initialize_tile_magic`), 5 inert systems activate with zero code changes:

| System | Trigger | Effect |
|---|---|---|
| CorruptionSpread | Tile corruption > 0.3 | Corruption bleeds to 4-adjacent neighbors |
| CorruptionTileEffect | Cat on tile with corruption > 0.1 | Mood penalty; herbs twist above 0.3 |
| ShadowFoxSpawn | Tile corruption > 0.7 | Shadow foxes emerge (up to 2 concurrent) |
| WardDecay | Ward entities exist | Corruption gives reason to place wards |
| PersonalCorruptionEffect | Cat corruption component > 0.3 | Mood drops, erratic behavior |

Secondary activations from mystery/special terrain:
- Dreamroot herbs spawn on FairyRing/StandingStone tiles
- SpiritCommunion action unlocks on special terrain
- `on_special_terrain` AI scoring path goes live

## Files Changed

| File | Change |
|---|---|
| `src/world_gen/special_tiles.rs` | **New** — placement algorithm, footprint stamping, tests |
| `src/world_gen/mod.rs` | Add `pub mod special_tiles;` |
| `src/resources/sim_constants.rs` | Add `WorldGenConstants` struct; add `world_gen` field to `SimConstants` |
| `src/plugins/setup.rs` | Reorder pipeline; insert `place_special_tiles` call |
| `src/main.rs` | Same reorder |
| `tests/integration.rs` | Add `place_special_tiles` call; add missing `initialize_tile_magic` call |

## Tests

All in `src/world_gen/special_tiles.rs`:

1. **`special_tiles_placed_on_grass_map`** — all-Grass 120×90 map; assert all 4 types present at target counts.
2. **`special_tiles_respect_spacing`** — assert every pair of anchors ≥ 15 manhattan distance.
3. **`ancient_ruin_far_from_colony`** — assert all AncientRuin tiles > 30 from colony_site.
4. **`deep_pool_near_water`** — map with Water strip; assert every DeepPool has ≥1 Water neighbor.
5. **`fairy_ring_is_hollow`** — assert center of 3×3 cluster is not FairyRing.
6. **`placement_deterministic`** — same seed → identical positions.
7. **`placement_on_real_terrain`** — Perlin-generated map; assert at least some of each type placed.
8. **`small_map_graceful_degradation`** — 30×30 map; assert no panic, may place fewer than target.

## Verification

After implementation:

1. `just check` — compiles, clippy clean
2. `just test` — all new + existing tests pass
3. `just run` — visual confirmation: special tiles appear on map, corruption spreads from ruins
4. `just score-track && just score-diff` — `features_active` should increase (5 newly active systems); `deaths_starvation` should not regress; watch for shadow fox deaths as new signal
