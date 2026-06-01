//! Ticket 101 — environmental quality influence-map sweep + feature
//! emission. Runs after `decay_building_condition` so per-tile reads of
//! `structure.condition` / `structure.cleanliness` are current.
//!
//! ## Systems
//!
//! - [`update_env_quality_maps`] — once per tick, rebuilds the five
//!   influence maps from terrain (`TileMap`), building entities
//!   (`Structure` + `Position`), unburied dead (`Dead`, `Without<Buried>`),
//!   and the current weather phase. Authoritative: cells are cleared at
//!   the top of the sweep so stale stamps don't persist when sources
//!   move or decay.
//!
//! - [`emit_env_quality_features`] — runs immediately after the sweep.
//!   Iterates living cats with `Position + Personality`, samples the
//!   four mood-relevant maps at each cat's tile, computes the combined
//!   modifier value via the shared
//!   [`combined_env_quality`](crate::resources::env_quality::combined_env_quality)
//!   helper, and records `Feature::EnvironmentalComfortPositive` /
//!   `Negative` if any cat clears the configured threshold. The
//!   modifier itself runs at score-time inside the pipeline; this
//!   companion system exists because `ScoreModifier::apply` is pure
//!   (no `SystemActivation` access) and a colony-level canary needs a
//!   different emission site.

use bevy_ecs::prelude::*;

use crate::components::building::{ConstructionSite, Structure, StructureType};
use crate::components::markers::Buried;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Position};
use crate::resources::env_quality::{combined_env_quality, stamp, EnvField};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::weather::WeatherState;
use crate::resources::{BeautyMap, CleanlinessMap, ComfortMap, CorruptionInfluenceMap, MysteryMap};

/// Rebuild the five env-quality influence maps from terrain, buildings,
/// unburied dead, and weather. Single sweep per tick.
#[allow(clippy::too_many_arguments)]
pub fn update_env_quality_maps(
    map: Res<TileMap>,
    buildings: Query<(&Structure, &Position), Without<ConstructionSite>>,
    dead: Query<&Position, (With<Dead>, Without<Buried>)>,
    weather: Res<WeatherState>,
    constants: Res<SimConstants>,
    mut comfort: ResMut<ComfortMap>,
    mut cleanliness: ResMut<CleanlinessMap>,
    mut beauty: ResMut<BeautyMap>,
    mut mystery: ResMut<MysteryMap>,
    mut corruption: ResMut<CorruptionInfluenceMap>,
) {
    let c = &constants.environmental_quality;

    comfort.field.clear();
    cleanliness.field.clear();
    beauty.field.clear();
    mystery.field.clear();
    corruption.field.clear();

    // --- 1. Terrain sweep — one pass over the full TileMap. ---
    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.get(x, y);
            stamp_terrain_comfort(&mut comfort.field, x, y, tile.terrain, c);
            stamp_terrain_cleanliness(&mut cleanliness.field, x, y, tile.terrain, c);
            stamp_terrain_beauty(&mut beauty.field, x, y, tile.terrain, tile.corruption, c);
            if tile.mystery > c.mystery_stamp_threshold {
                stamp(
                    &mut mystery.field,
                    x,
                    y,
                    tile.mystery,
                    c.mystery_stamp_radius,
                );
            }
            if tile.corruption > c.corruption_stamp_threshold {
                stamp(
                    &mut corruption.field,
                    x,
                    y,
                    tile.corruption,
                    c.corruption_stamp_radius,
                );
            }
        }
    }

    // --- 2. Buildings sweep — comfort + beauty + cleanliness contributions. ---
    for (structure, pos) in &buildings {
        let condition = structure.condition.clamp(0.0, 1.0);
        match structure.kind {
            StructureType::Den => {
                stamp(
                    &mut comfort.field,
                    pos.x(),
                    pos.y(),
                    c.comfort_building_den_peak * condition,
                    c.comfort_building_den_radius,
                );
                stamp(
                    &mut beauty.field,
                    pos.x(),
                    pos.y(),
                    c.beauty_building_den_peak * condition,
                    c.beauty_building_den_radius,
                );
            }
            StructureType::Hearth => {
                stamp(
                    &mut comfort.field,
                    pos.x(),
                    pos.y(),
                    c.comfort_building_hearth_peak * condition,
                    c.comfort_building_hearth_radius,
                );
                stamp(
                    &mut beauty.field,
                    pos.x(),
                    pos.y(),
                    c.beauty_building_hearth_peak * condition,
                    c.beauty_building_hearth_radius,
                );
            }
            StructureType::Stores => {
                stamp(
                    &mut comfort.field,
                    pos.x(),
                    pos.y(),
                    c.comfort_building_stores_peak * condition,
                    c.comfort_building_stores_radius,
                );
            }
            StructureType::Workshop => {
                stamp(
                    &mut comfort.field,
                    pos.x(),
                    pos.y(),
                    c.comfort_building_workshop_peak * condition,
                    c.comfort_building_workshop_radius,
                );
            }
            StructureType::Garden => {
                stamp(
                    &mut comfort.field,
                    pos.x(),
                    pos.y(),
                    c.comfort_building_garden_peak * condition,
                    c.comfort_building_garden_radius,
                );
                stamp(
                    &mut beauty.field,
                    pos.x(),
                    pos.y(),
                    c.beauty_building_garden_peak * condition,
                    c.beauty_building_garden_radius,
                );
            }
            StructureType::WardPost => {
                stamp(
                    &mut comfort.field,
                    pos.x(),
                    pos.y(),
                    c.comfort_building_ward_post_peak * condition,
                    c.comfort_building_ward_post_radius,
                );
            }
            StructureType::Kitchen
            | StructureType::Watchtower
            | StructureType::Wall
            | StructureType::Gate
            | StructureType::Midden
            | StructureType::DryingRack
            | StructureType::SmokingRack
            | StructureType::TanningFrame => {
                // Buildings without a comfort / beauty contribution
                // in the 101 spec. Cleanliness penalty for dirty
                // buildings still applies below.
            }
        }

        // Cleanliness penalty for buildings below the dirty threshold —
        // applies to any building kind that can accumulate filth.
        if structure.cleanliness < constants.buildings.dirty_threshold {
            let dirty_factor = (1.0 - structure.cleanliness).clamp(0.0, 1.0);
            stamp(
                &mut cleanliness.field,
                pos.x(),
                pos.y(),
                c.cleanliness_dirty_building_peak * dirty_factor,
                c.cleanliness_dirty_building_radius,
            );
        }
    }

    // --- 3. Unburied-dead sweep — cleanliness penalty. ---
    for pos in &dead {
        stamp(
            &mut cleanliness.field,
            pos.x(),
            pos.y(),
            c.cleanliness_corpse_peak,
            c.cleanliness_corpse_radius,
        );
    }

    // --- 4. Weather overlay — flat additive offset to comfort cells. ---
    let weather_delta = weather.current.comfort_modifier();
    if weather_delta != 0.0 {
        comfort.field.add_global(weather_delta);
    }

    // --- 5. Final clamp — every cell to [-1.0, 1.0]. ---
    // The per-stamp clamp inside `stamp` already keeps cells in range,
    // but `add_global` clamps too — leave the explicit clamp_all as
    // belt-and-suspenders in case a future source forgets.
    comfort.field.clamp_all();
    cleanliness.field.clamp_all();
    beauty.field.clamp_all();
    mystery.field.clamp_all();
    corruption.field.clamp_all();
}

fn stamp_terrain_comfort(
    field: &mut EnvField,
    x: i32,
    y: i32,
    terrain: Terrain,
    c: &crate::resources::sim_constants::EnvironmentalQualityConstants,
) {
    let peak = match terrain {
        Terrain::FairyRing => c.comfort_terrain_fairy_ring,
        Terrain::LightForest => c.comfort_terrain_light_forest,
        Terrain::DenseForest => c.comfort_terrain_dense_forest,
        Terrain::Sand => c.comfort_terrain_sand,
        Terrain::Mud => c.comfort_terrain_mud,
        Terrain::Rock => c.comfort_terrain_rock,
        _ => 0.0,
    };
    if peak != 0.0 {
        stamp(field, x, y, peak, 0.0);
    }
}

fn stamp_terrain_cleanliness(
    field: &mut EnvField,
    x: i32,
    y: i32,
    terrain: Terrain,
    c: &crate::resources::sim_constants::EnvironmentalQualityConstants,
) {
    if terrain == Terrain::Mud {
        stamp(field, x, y, c.cleanliness_terrain_mud, 0.0);
    }
}

fn stamp_terrain_beauty(
    field: &mut EnvField,
    x: i32,
    y: i32,
    terrain: Terrain,
    corruption: f32,
    c: &crate::resources::sim_constants::EnvironmentalQualityConstants,
) {
    match terrain {
        Terrain::FairyRing => stamp(
            field,
            x,
            y,
            c.beauty_terrain_fairy_ring_peak,
            c.beauty_terrain_fairy_ring_radius,
        ),
        Terrain::StandingStone => stamp(
            field,
            x,
            y,
            c.beauty_terrain_standing_stone_peak,
            c.beauty_terrain_standing_stone_radius,
        ),
        Terrain::DeepPool => stamp(
            field,
            x,
            y,
            c.beauty_terrain_deep_pool_peak,
            c.beauty_terrain_deep_pool_radius,
        ),
        Terrain::Garden => stamp(
            field,
            x,
            y,
            c.beauty_terrain_garden_peak,
            c.beauty_terrain_garden_radius,
        ),
        Terrain::AncientRuin => stamp(field, x, y, c.beauty_terrain_ancient_ruin, 0.0),
        _ => {}
    }
    if corruption > 0.0 {
        let suppression = -corruption * c.beauty_corruption_suppression;
        if suppression != 0.0 {
            stamp(field, x, y, suppression, 0.0);
        }
    }
}

/// Companion canary system. Runs after `update_env_quality_maps`,
/// samples each living cat's tile, runs the same combine math the
/// modifier uses, and records `Feature::EnvironmentalComfortPositive` /
/// `Negative` if any cat clears the configured threshold.
pub fn emit_env_quality_features(
    cats: Query<(&Position, &Personality), Without<Dead>>,
    comfort: Res<ComfortMap>,
    cleanliness: Res<CleanlinessMap>,
    beauty: Res<BeautyMap>,
    mystery: Res<MysteryMap>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.environmental_quality;
    let threshold = c.feature_emit_threshold;
    let mut saw_positive = false;
    let mut saw_negative = false;
    for (pos, personality) in &cats {
        let value = combined_env_quality(
            comfort.get(pos.x(), pos.y()),
            cleanliness.get(pos.x(), pos.y()),
            beauty.get(pos.x(), pos.y()),
            mystery.get(pos.x(), pos.y()),
            personality,
            c,
        );
        if value >= threshold {
            saw_positive = true;
        }
        if value <= -threshold {
            saw_negative = true;
        }
        if saw_positive && saw_negative {
            break;
        }
    }
    if saw_positive {
        activation.record(Feature::EnvironmentalComfortPositive);
    }
    if saw_negative {
        activation.record(Feature::EnvironmentalComfortNegative);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::Position;
    use crate::resources::map::{Tile, TileMap};
    use crate::resources::weather::{Weather, WeatherState};

    fn build_minimal_world() -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        world.insert_resource(WeatherState {
            current: Weather::Clear,
            ticks_until_change: 100,
        });
        let mut tile_map = TileMap::new(20, 20, Terrain::Grass);
        // Plant a small terrain signal so the sweep stamps something
        // recognisable.
        *tile_map.get_mut(10, 10) = Tile {
            terrain: Terrain::FairyRing,
            corruption: 0.0,
            mystery: 0.8,
        };
        world.insert_resource(tile_map);
        world.insert_resource(ComfortMap::new(20, 20));
        world.insert_resource(CleanlinessMap::new(20, 20));
        world.insert_resource(BeautyMap::new(20, 20));
        world.insert_resource(MysteryMap::new(20, 20));
        world.insert_resource(CorruptionInfluenceMap::new(20, 20));
        world.insert_resource(SystemActivation::default());
        let hearth = world
            .spawn((
                Structure {
                    kind: StructureType::Hearth,
                    condition: 1.0,
                    cleanliness: 1.0,
                    size: (1, 1),
                },
                Position::new(5, 5),
            ))
            .id();
        (world, hearth)
    }

    #[test]
    fn update_stamps_terrain_and_buildings() {
        let (mut world, _) = build_minimal_world();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_env_quality_maps);
        schedule.run(&mut world);

        let comfort = world.resource::<ComfortMap>();
        // Fairy ring tile carries the on-tile comfort base, plus zero
        // building contribution.
        assert!(comfort.get(10, 10) > 0.0);
        // Hearth radiates comfort within radius 3.
        assert!(comfort.get(5, 5) > 0.0);
        assert!(comfort.get(6, 5) > 0.0);

        let beauty = world.resource::<BeautyMap>();
        assert!(beauty.get(10, 10) > 0.0);
        // Hearth aesthetic upkeep adds beauty.
        assert!(beauty.get(5, 5) > 0.0);

        let mystery = world.resource::<MysteryMap>();
        assert!(mystery.get(10, 10) > 0.0);
        // Mystery stamps outward, so adjacent tiles also see resonance.
        assert!(mystery.get(11, 10) > 0.0);
    }

    #[test]
    fn update_idempotent_across_runs() {
        let (mut world, _) = build_minimal_world();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_env_quality_maps);
        schedule.run(&mut world);
        let snapshot = world.resource::<ComfortMap>().field.marks.clone();
        schedule.run(&mut world);
        let after = world.resource::<ComfortMap>().field.marks.clone();
        // Re-running the sweep with the same world produces the same
        // map — `clear()` at the top prevents accumulation.
        assert_eq!(snapshot, after);
    }

    #[test]
    fn unburied_dead_stamps_cleanliness_negative() {
        let (mut world, _) = build_minimal_world();
        world.spawn((
            Dead {
                tick: 0,
                cause: crate::components::physical::DeathCause::OldAge,
            },
            Position::new(15, 15),
        ));
        let mut schedule = Schedule::default();
        schedule.add_systems(update_env_quality_maps);
        schedule.run(&mut world);
        let cleanliness = world.resource::<CleanlinessMap>();
        assert!(cleanliness.get(15, 15) < 0.0);
    }

    #[test]
    fn weather_storm_pushes_comfort_negative_globally() {
        let (mut world, _) = build_minimal_world();
        // Replace the weather state.
        world.insert_resource(WeatherState {
            current: Weather::Storm,
            ticks_until_change: 100,
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(update_env_quality_maps);
        schedule.run(&mut world);
        let comfort = world.resource::<ComfortMap>();
        // A grass tile far from the hearth gets only the global storm
        // overlay (which is negative).
        assert!(comfort.get(0, 0) < 0.0);
    }

    #[test]
    fn emit_features_when_cat_near_hearth() {
        let (mut world, _) = build_minimal_world();
        // Drop a high-warmth cat onto the hearth tile.
        let mut p = Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 1.0,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        };
        // Quieten the unused-mut warning while documenting intent.
        p.warmth = 1.0;
        world.spawn((Position::new(5, 5), p));

        let mut schedule = Schedule::default();
        schedule.add_systems((update_env_quality_maps, emit_env_quality_features).chain());
        schedule.run(&mut world);

        let activation = world.resource::<SystemActivation>();
        assert!(
            !activation
                .dead_features()
                .contains(&Feature::EnvironmentalComfortPositive),
            "Cat at the hearth should trip the positive canary"
        );
    }
}
