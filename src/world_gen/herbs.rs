use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::{Harvestable, Herb, HerbKind, Seasonal};
use crate::components::physical::Position;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::time::Season;

// ---------------------------------------------------------------------------
// Herb spawning at world gen
// ---------------------------------------------------------------------------

/// Per-tile data snapshot used to decide herb placement without holding a
/// borrow on the World.
struct TileInfo {
    x: i32,
    y: i32,
    terrain: Terrain,
    mystery: f32,
    forest_edge: bool,
}

/// Scatter herb entities across the map based on terrain affinity and density.
///
/// Called once during world generation. Each herb gets a `Seasonal` component
/// that controls when its `Harvestable` marker is active.
pub fn spawn_herbs(world: &mut World, current_season: Season) {
    // Phase 1: snapshot all tile data (releases the map borrow).
    let tile_info: Vec<TileInfo> = {
        let map = world.resource::<TileMap>();
        let mut info = Vec::with_capacity((map.width * map.height) as usize);
        for y in 0..map.height {
            for x in 0..map.width {
                let tile = map.get(x, y);
                info.push(TileInfo {
                    x,
                    y,
                    terrain: tile.terrain,
                    mystery: tile.mystery,
                    forest_edge: is_forest_edge(x, y, map),
                });
            }
        }
        info
    };

    // Phase 2: use rng to decide which tiles get herbs.
    let herb_spawns: Vec<(HerbKind, i32, i32, bool)> = {
        let mut rng = world.resource_mut::<crate::resources::rng::SimRng>();
        let mut spawns = Vec::new();

        for ti in &tile_info {
            for kind in ALL_HERB_KINDS {
                if !kind.spawn_terrains().contains(&ti.terrain) {
                    continue;
                }
                if kind == HerbKind::Thornbriar && !ti.forest_edge {
                    continue;
                }
                if rng.rng.random::<f32>() < kind.spawn_density() {
                    spawns.push((kind, ti.x, ti.y, ti.mystery > 0.5));
                }
            }
        }
        spawns
    };

    // Phase 3: spawn herb entities.
    for (kind, x, y, magical) in herb_spawns {
        let available = kind.available_seasons().to_vec();
        let in_season = available.contains(&current_season);

        let mut ec = world.spawn((
            Herb {
                kind,
                magical,
                twisted: false,
            },
            Position::new(x, y),
            Seasonal { available },
        ));
        if in_season {
            ec.insert(Harvestable);
        }
    }
}

/// Set initial corruption on AncientRuin tiles and mystery on special tiles.
pub fn initialize_tile_magic(map: &mut TileMap, rng: &mut impl Rng) {
    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.get_mut(x, y);
            match tile.terrain {
                Terrain::AncientRuin => {
                    tile.corruption = rng.random_range(0.5..0.8);
                    tile.mystery = rng.random_range(0.3..0.6);
                }
                Terrain::FairyRing => {
                    tile.mystery = rng.random_range(0.7..1.0);
                }
                Terrain::StandingStone => {
                    tile.mystery = rng.random_range(0.6..0.9);
                }
                Terrain::DeepPool => {
                    tile.mystery = rng.random_range(0.4..0.7);
                }
                _ => {}
            }
        }
    }
}

const ALL_HERB_KINDS: [HerbKind; 5] = [
    HerbKind::HealingMoss,
    HerbKind::Moonpetal,
    HerbKind::Calmroot,
    HerbKind::Thornbriar,
    HerbKind::Dreamroot,
];

/// Check if (x, y) is a forest tile adjacent to a non-forest tile.
fn is_forest_edge(x: i32, y: i32, map: &TileMap) -> bool {
    let deltas = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    for (dx, dy) in deltas {
        let nx = x + dx;
        let ny = y + dy;
        if map.in_bounds(nx, ny) {
            let neighbor = map.get(nx, ny).terrain;
            if !matches!(neighbor, Terrain::LightForest | Terrain::DenseForest) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::rng::SimRng;

    #[test]
    fn initialize_tile_magic_sets_ruin_corruption() {
        let mut map = TileMap::new(10, 10, Terrain::Grass);
        map.set(3, 3, Terrain::AncientRuin);
        map.set(5, 5, Terrain::FairyRing);

        let mut rng = SimRng::new(42);
        initialize_tile_magic(&mut map, &mut rng.rng);

        assert!(
            map.get(3, 3).corruption >= 0.5,
            "AncientRuin should have corruption >= 0.5"
        );
        assert!(
            map.get(5, 5).mystery >= 0.7,
            "FairyRing should have mystery >= 0.7"
        );
        assert_eq!(
            map.get(0, 0).corruption, 0.0,
            "grass tile should have no corruption"
        );
    }

    #[test]
    fn spawn_herbs_creates_entities() {
        let mut world = World::new();
        let mut rng = SimRng::new(42);
        let map = crate::world_gen::terrain::generate_terrain(40, 30, &mut rng.rng);
        world.insert_resource(map);
        world.insert_resource(rng);

        spawn_herbs(&mut world, Season::Summer);

        let herb_count = world.query::<&Herb>().iter(&world).count();
        assert!(
            herb_count > 0,
            "should have spawned at least some herbs on a 40x30 map"
        );
    }
}
