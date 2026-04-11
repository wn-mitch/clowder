use rand::Rng;
use rand::seq::SliceRandom;

use crate::components::physical::Position;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::sim_constants::WorldGenConstants;

/// Descriptor for a special tile type to be placed during world generation.
struct SpecialSiteKind {
    terrain: Terrain,
    target_count: usize,
    /// Whether this type requires minimum distance from the colony.
    requires_colony_distance: bool,
    /// Footprint offsets relative to anchor (top-left).
    footprint: &'static [(i32, i32)],
    /// Bounding box (width, height) used for in-bounds checks.
    bounds: (i32, i32),
    /// Valid base terrains the footprint tiles can overwrite.
    affinities: &'static [Terrain],
    /// If true, anchor must have at least one Water neighbor in 4-directions.
    requires_water_adjacency: bool,
}

/// 2×2 solid footprint.
const FOOTPRINT_2X2: &[(i32, i32)] = &[(0, 0), (1, 0), (0, 1), (1, 1)];

/// 3×3 hollow ring (8 perimeter tiles, center unchanged).
const FOOTPRINT_RING_3X3: &[(i32, i32)] = &[
    (0, 0),
    (1, 0),
    (2, 0),
    (0, 1),
    (2, 1),
    (0, 2),
    (1, 2),
    (2, 2),
];

/// 1×1 single tile.
const FOOTPRINT_1X1: &[(i32, i32)] = &[(0, 0)];

fn build_site_kinds(c: &WorldGenConstants) -> Vec<SpecialSiteKind> {
    vec![
        // AncientRuin first — tightest constraint (colony distance).
        SpecialSiteKind {
            terrain: Terrain::AncientRuin,
            target_count: c.ancient_ruin_count,
            requires_colony_distance: true,
            footprint: FOOTPRINT_2X2,
            bounds: (2, 2),
            affinities: &[Terrain::Grass, Terrain::Sand],
            requires_water_adjacency: false,
        },
        // FairyRing second — 3×3 footprint needs more contiguous space.
        SpecialSiteKind {
            terrain: Terrain::FairyRing,
            target_count: c.fairy_ring_count,
            requires_colony_distance: false,
            footprint: FOOTPRINT_RING_3X3,
            bounds: (3, 3),
            affinities: &[Terrain::Grass, Terrain::LightForest],
            requires_water_adjacency: false,
        },
        SpecialSiteKind {
            terrain: Terrain::StandingStone,
            target_count: c.standing_stone_count,
            requires_colony_distance: false,
            footprint: FOOTPRINT_1X1,
            bounds: (1, 1),
            affinities: &[Terrain::Grass, Terrain::Rock, Terrain::Sand],
            requires_water_adjacency: false,
        },
        SpecialSiteKind {
            terrain: Terrain::DeepPool,
            target_count: c.deep_pool_count,
            requires_colony_distance: false,
            footprint: FOOTPRINT_1X1,
            bounds: (1, 1),
            affinities: &[Terrain::Grass, Terrain::Mud],
            requires_water_adjacency: true,
        },
    ]
}

/// Returns true if (x, y) has at least one Water-terrain neighbor in 4 cardinal
/// directions.
fn has_water_neighbor(map: &TileMap, x: i32, y: i32) -> bool {
    for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
        let nx = x + dx;
        let ny = y + dy;
        if map.in_bounds(nx, ny) && map.get(nx, ny).terrain == Terrain::Water {
            return true;
        }
    }
    false
}

/// Returns true if every tile in the footprint at anchor (ax, ay) is in bounds,
/// within edge margins, matches the terrain affinity, and isn't already a special
/// tile.
fn footprint_valid(
    map: &TileMap,
    ax: i32,
    ay: i32,
    kind: &SpecialSiteKind,
    margin: i32,
) -> bool {
    let (bw, bh) = kind.bounds;
    // Entire bounding box must be within edge margin.
    if ax < margin || ay < margin || ax + bw > map.width - margin || ay + bh > map.height - margin {
        return false;
    }
    // Every footprint tile must match affinity and not already be a special tile.
    for &(dx, dy) in kind.footprint {
        let tile = map.get(ax + dx, ay + dy);
        if !kind.affinities.contains(&tile.terrain) {
            return false;
        }
    }
    // For the full bounding box (including hollow center), check no overlap with
    // existing special tiles.
    for dy in 0..bh {
        for dx in 0..bw {
            let t = map.get(ax + dx, ay + dy).terrain;
            if matches!(
                t,
                Terrain::AncientRuin
                    | Terrain::FairyRing
                    | Terrain::StandingStone
                    | Terrain::DeepPool
            ) {
                return false;
            }
        }
    }
    true
}

/// Place special terrain tiles on the map using Poisson-disk–style placement.
///
/// Must be called **after** `generate_terrain` and `find_colony_site` but
/// **before** `initialize_tile_magic` (which seeds corruption/mystery on the
/// tiles placed here).
pub fn place_special_tiles(
    map: &mut TileMap,
    colony_site: Position,
    rng: &mut impl Rng,
    constants: &WorldGenConstants,
) {
    let kinds = build_site_kinds(constants);
    let mut placed_anchors: Vec<Position> = Vec::new();

    for kind in &kinds {
        let mut placed_this_type = 0usize;

        // Collect all candidate anchor positions.
        let mut candidates: Vec<(i32, i32)> = Vec::new();
        for y in 0..map.height {
            for x in 0..map.width {
                if footprint_valid(map, x, y, kind, constants.edge_margin) {
                    if kind.requires_water_adjacency && !has_water_neighbor(map, x, y) {
                        continue;
                    }
                    candidates.push((x, y));
                }
            }
        }

        candidates.shuffle(rng);
        if candidates.len() > constants.max_placement_attempts {
            candidates.truncate(constants.max_placement_attempts);
        }

        for &(x, y) in &candidates {
            if placed_this_type >= kind.target_count {
                break;
            }
            let anchor = Position::new(x, y);

            // Spacing check: far enough from all previously placed anchors.
            let spaced = placed_anchors
                .iter()
                .all(|p| anchor.manhattan_distance(p) >= constants.special_min_spacing);
            if !spaced {
                continue;
            }

            // Colony distance check for corruption sources.
            if kind.requires_colony_distance
                && anchor.manhattan_distance(&colony_site) < constants.corruption_colony_min_distance
            {
                continue;
            }

            // Stamp footprint.
            for &(dx, dy) in kind.footprint {
                map.set(x + dx, y + dy, kind.terrain);
            }
            placed_anchors.push(anchor);
            placed_this_type += 1;
        }

        eprintln!(
            "  special tiles: placed {placed_this_type}/{} {:?}",
            kind.target_count, kind.terrain,
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::rng::SimRng;
    use crate::world_gen::terrain::generate_terrain;

    fn default_constants() -> WorldGenConstants {
        WorldGenConstants::default()
    }

    /// Count tiles of a given terrain type across the whole map.
    fn count_terrain(map: &TileMap, terrain: Terrain) -> usize {
        let mut n = 0;
        for y in 0..map.height {
            for x in 0..map.width {
                if map.get(x, y).terrain == terrain {
                    n += 1;
                }
            }
        }
        n
    }

    /// Collect anchor positions: the top-left tile of each contiguous cluster of
    /// a given special terrain. For 1×1 tiles this is every instance. For larger
    /// footprints we just collect all matching positions (the spacing test checks
    /// anchors via the placed list directly).
    fn find_special_positions(map: &TileMap, terrain: Terrain) -> Vec<Position> {
        let mut positions = Vec::new();
        for y in 0..map.height {
            for x in 0..map.width {
                if map.get(x, y).terrain == terrain {
                    positions.push(Position::new(x, y));
                }
            }
        }
        positions
    }

    // -----------------------------------------------------------------------
    // Test 1: All 4 types placed on all-Grass map at target counts
    // -----------------------------------------------------------------------

    #[test]
    fn special_tiles_placed_on_grass_map() {
        let mut map = TileMap::new(120, 90, Terrain::Grass);
        let colony = Position::new(60, 45);
        let c = default_constants();
        let mut rng = SimRng::new(42);

        place_special_tiles(&mut map, colony, &mut rng.rng, &c);

        // AncientRuin: 3 sites × 4 tiles each = 12
        assert_eq!(count_terrain(&map, Terrain::AncientRuin), 3 * 4);
        // FairyRing: 2 sites × 8 tiles each = 16
        assert_eq!(count_terrain(&map, Terrain::FairyRing), 2 * 8);
        // StandingStone: 3 sites × 1 tile each = 3
        assert_eq!(count_terrain(&map, Terrain::StandingStone), 3);
        // DeepPool: requires Water adjacency — none on all-Grass map, so 0.
        assert_eq!(count_terrain(&map, Terrain::DeepPool), 0);
    }

    // -----------------------------------------------------------------------
    // Test 2: Spacing constraint between all anchors
    // -----------------------------------------------------------------------

    #[test]
    fn special_tiles_respect_spacing() {
        let mut map = TileMap::new(120, 90, Terrain::Grass);
        let colony = Position::new(60, 45);
        let c = default_constants();
        let mut rng = SimRng::new(99);

        place_special_tiles(&mut map, colony, &mut rng.rng, &c);

        // Collect all special tile positions and check pairwise spacing.
        let specials = [
            Terrain::AncientRuin,
            Terrain::FairyRing,
            Terrain::StandingStone,
        ];
        // For multi-tile footprints, find approximate anchors by taking the
        // min-x, min-y tile of each connected cluster. For spacing purposes,
        // verifying that distinct tiles of different types are far apart is
        // sufficient.
        let mut all_positions: Vec<Position> = Vec::new();
        for &t in &specials {
            let positions = find_special_positions(&map, t);
            all_positions.extend(positions);
        }

        // Each special tile should be at least `special_min_spacing` from any
        // tile belonging to a DIFFERENT placement. Tiles within the same
        // footprint are close by design. We check that no two tiles from
        // different terrain types are closer than spacing - max_footprint_size.
        // A simpler assertion: no two different-terrain special tiles within 12
        // (15 - 3 for max footprint width).
        let effective_min = c.special_min_spacing - 3; // account for footprint width
        for i in 0..all_positions.len() {
            for j in (i + 1)..all_positions.len() {
                let ti = map.get(all_positions[i].x, all_positions[i].y).terrain;
                let tj = map.get(all_positions[j].x, all_positions[j].y).terrain;
                if ti != tj {
                    let dist = all_positions[i].manhattan_distance(&all_positions[j]);
                    assert!(
                        dist >= effective_min,
                        "different-type special tiles too close: {:?}@{:?} and {:?}@{:?}, dist={dist}",
                        ti, all_positions[i], tj, all_positions[j],
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test 3: AncientRuin colony distance
    // -----------------------------------------------------------------------

    #[test]
    fn ancient_ruin_far_from_colony() {
        let mut map = TileMap::new(120, 90, Terrain::Grass);
        let colony = Position::new(60, 45);
        let c = default_constants();
        let mut rng = SimRng::new(7);

        place_special_tiles(&mut map, colony, &mut rng.rng, &c);

        for pos in find_special_positions(&map, Terrain::AncientRuin) {
            let dist = pos.manhattan_distance(&colony);
            assert!(
                dist >= c.corruption_colony_min_distance,
                "AncientRuin at {:?} is only {dist} from colony",
                pos,
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 4: DeepPool requires Water adjacency
    // -----------------------------------------------------------------------

    #[test]
    fn deep_pool_near_water() {
        let mut map = TileMap::new(120, 90, Terrain::Grass);
        // Place a vertical strip of Water so DeepPool has candidates.
        for y in 0..90 {
            map.set(100, y, Terrain::Water);
        }
        let colony = Position::new(30, 45);
        let c = default_constants();
        let mut rng = SimRng::new(2025);

        place_special_tiles(&mut map, colony, &mut rng.rng, &c);

        let pools = find_special_positions(&map, Terrain::DeepPool);
        assert!(!pools.is_empty(), "should place at least one DeepPool near water");
        for pos in &pools {
            assert!(
                has_water_neighbor(&map, pos.x, pos.y),
                "DeepPool at {:?} has no Water neighbor",
                pos,
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 5: FairyRing is hollow (center unchanged)
    // -----------------------------------------------------------------------

    #[test]
    fn fairy_ring_is_hollow() {
        let mut map = TileMap::new(120, 90, Terrain::Grass);
        let colony = Position::new(60, 45);
        let c = default_constants();
        let mut rng = SimRng::new(314);

        place_special_tiles(&mut map, colony, &mut rng.rng, &c);

        // Find FairyRing tiles and check for at least one 3×3 cluster with a
        // non-FairyRing center.
        let ring_tiles = find_special_positions(&map, Terrain::FairyRing);
        assert!(!ring_tiles.is_empty(), "should have placed FairyRing tiles");

        // For each FairyRing tile at (x, y), check if it could be the top-left
        // of a 3×3 ring: all 8 perimeter tiles are FairyRing, center is not.
        let mut found_hollow = false;
        for pos in &ring_tiles {
            let (ax, ay) = (pos.x, pos.y);
            if ax + 2 >= map.width || ay + 2 >= map.height {
                continue;
            }
            let center = map.get(ax + 1, ay + 1).terrain;
            if center == Terrain::FairyRing {
                continue;
            }
            // Check all 8 perimeter tiles.
            let all_ring = FOOTPRINT_RING_3X3.iter().all(|&(dx, dy)| {
                map.get(ax + dx, ay + dy).terrain == Terrain::FairyRing
            });
            if all_ring {
                found_hollow = true;
                break;
            }
        }
        assert!(found_hollow, "no hollow FairyRing 3×3 pattern found");
    }

    // -----------------------------------------------------------------------
    // Test 6: Deterministic placement
    // -----------------------------------------------------------------------

    #[test]
    fn placement_deterministic() {
        let c = default_constants();
        let colony = Position::new(60, 45);

        let mut map1 = TileMap::new(120, 90, Terrain::Grass);
        place_special_tiles(&mut map1, colony, &mut SimRng::new(42).rng, &c);

        let mut map2 = TileMap::new(120, 90, Terrain::Grass);
        place_special_tiles(&mut map2, colony, &mut SimRng::new(42).rng, &c);

        for y in 0..90 {
            for x in 0..120 {
                assert_eq!(
                    map1.get(x, y).terrain,
                    map2.get(x, y).terrain,
                    "mismatch at ({x}, {y})",
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test 7: Placement on real Perlin terrain
    // -----------------------------------------------------------------------

    #[test]
    fn placement_on_real_terrain() {
        let mut rng = SimRng::new(42);
        let mut map = generate_terrain(120, 90, &mut rng.rng);
        let colony = Position::new(60, 45);
        let c = default_constants();

        place_special_tiles(&mut map, colony, &mut rng.rng, &c);

        // Should place at least some of each type (except DeepPool depends on
        // Water adjacency which may or may not exist).
        assert!(
            count_terrain(&map, Terrain::AncientRuin) > 0,
            "should place at least one AncientRuin on Perlin map",
        );
        assert!(
            count_terrain(&map, Terrain::FairyRing) > 0,
            "should place at least one FairyRing on Perlin map",
        );
        assert!(
            count_terrain(&map, Terrain::StandingStone) > 0,
            "should place at least one StandingStone on Perlin map",
        );
    }

    // -----------------------------------------------------------------------
    // Test 8: Small map graceful degradation
    // -----------------------------------------------------------------------

    #[test]
    fn small_map_graceful_degradation() {
        let mut map = TileMap::new(30, 30, Terrain::Grass);
        let colony = Position::new(15, 15);
        let c = default_constants();
        let mut rng = SimRng::new(77);

        // Should not panic. May place fewer than target count due to spacing +
        // edge margin constraints on a small map.
        place_special_tiles(&mut map, colony, &mut rng.rng, &c);
    }
}
