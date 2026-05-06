use bevy_ecs::prelude::*;

use crate::components::building::{ConstructionSite, GateState, Structure, StructureType};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::resources::food::FoodStores;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{Season, SimConfig, TimeScale, TimeState};
use crate::resources::weather::{Weather, WeatherState};

// ---------------------------------------------------------------------------
// §4 colony-scoped building/food marker predicates
// ---------------------------------------------------------------------------

/// Colony-scoped boolean predicates derived from building and food state.
/// Computed once per tick by [`scan_colony_buildings`] and consumed by
/// both scoring systems (`goap.rs`, `disposition.rs`) to populate the
/// `MarkerSnapshot` without duplicating predicate logic.
pub struct ColonyBuildingState {
    pub has_construction_site: bool,
    pub has_damaged_building: bool,
    pub has_garden: bool,
    pub has_functional_kitchen: bool,
}

/// Single-pass scan over the building query to derive all colony-scoped
/// building predicates. Replaces four separate `.any()` calls that were
/// previously duplicated across `goap.rs` and `disposition.rs`.
pub fn scan_colony_buildings<'a>(
    buildings: impl Iterator<Item = (&'a Structure, Option<&'a ConstructionSite>)>,
    damaged_threshold: f32,
) -> ColonyBuildingState {
    let mut state = ColonyBuildingState {
        has_construction_site: false,
        has_damaged_building: false,
        has_garden: false,
        has_functional_kitchen: false,
    };
    for (structure, site) in buildings {
        if site.is_some() {
            state.has_construction_site = true;
        } else {
            if structure.condition < damaged_threshold {
                state.has_damaged_building = true;
            }
            if structure.kind == StructureType::Garden {
                state.has_garden = true;
            }
            if structure.kind == StructureType::Kitchen && structure.effectiveness() > 0.0 {
                state.has_functional_kitchen = true;
            }
        }
    }
    state
}

/// Manhattan-nearest active construction site to `cat_pos`. `None`
/// when no site exists. Read by the §L2.10.7 Build spatial axis via
/// [`crate::ai::considerations::LandmarkAnchor::NearestConstructionSite`].
/// Run per-cat in the scoring builder.
pub fn nearest_construction_site<'a>(
    buildings: impl Iterator<Item = (&'a Structure, &'a Position, Option<&'a ConstructionSite>)>,
    cat_pos: Position,
) -> Option<Position> {
    let mut best: Option<(Position, i32)> = None;
    for (structure, anchor, site) in buildings {
        if site.is_none() {
            continue;
        }
        let center = structure.center(anchor);
        let d = cat_pos.manhattan_distance(&center);
        if best.is_none_or(|(_, cur)| d < cur) {
            best = Some((center, d));
        }
    }
    best.map(|(pos, _)| pos)
}

// ---------------------------------------------------------------------------
// apply_building_effects
// ---------------------------------------------------------------------------

/// Each tick, completed buildings provide passive bonuses to nearby cats.
///
/// Runs after `detect_threats` and before `decay_needs` so that building
/// bonuses are applied before needs decay subtracts from them.
#[allow(clippy::too_many_arguments)]
pub fn apply_building_effects(
    buildings: Query<(&Structure, &Position), Without<ConstructionSite>>,
    mut cats: Query<(&Position, &mut Needs), Without<Dead>>,
    mut food: ResMut<FoodStores>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    time_scale: Res<TimeScale>,
    weather: Res<WeatherState>,
    constants: Res<SimConstants>,
) {
    let b = &constants.buildings;
    let den_temperature_bonus = b.den_temperature_bonus.per_tick(&time_scale);
    let den_safety_bonus = b.den_safety_bonus.per_tick(&time_scale);
    let hearth_social_bonus = b.hearth_social_bonus.per_tick(&time_scale);
    let hearth_temperature_bonus_cold = b.hearth_temperature_bonus_cold.per_tick(&time_scale);
    let dirty_temperature_drain = b.dirty_temperature_drain.per_tick(&time_scale);

    let season = time.season(&config);
    let is_winter = season == Season::Winter;
    let is_cold = is_winter
        || matches!(
            weather.current,
            Weather::Snow | Weather::Storm | Weather::Wind
        );

    // Reset spoilage multiplier each tick; Stores will set it if functional.
    food.spoilage_multiplier = 1.0;

    for (structure, building_pos) in &buildings {
        let eff = structure.effectiveness();
        if eff <= 0.0 {
            continue;
        }

        let center = structure.center(building_pos);

        match structure.kind {
            StructureType::Den => {
                for (cat_pos, mut needs) in &mut cats {
                    if cat_pos.manhattan_distance(&center) <= b.den_effect_radius {
                        needs.temperature =
                            (needs.temperature + den_temperature_bonus * eff).min(1.0);
                        needs.safety = (needs.safety + den_safety_bonus * eff).min(1.0);
                    }
                }
            }
            StructureType::Hearth => {
                for (cat_pos, mut needs) in &mut cats {
                    if cat_pos.manhattan_distance(&center) <= b.hearth_effect_radius {
                        needs.social = (needs.social + hearth_social_bonus * eff).min(1.0);
                        if is_cold {
                            needs.temperature = (needs.temperature
                                + hearth_temperature_bonus_cold * eff)
                                .min(1.0);
                        }
                    }
                }
            }
            StructureType::Stores => {
                food.spoilage_multiplier = b.stores_spoilage_multiplier;
            }
            // Workshop, Watchtower, WardPost, Wall, Gate, Garden:
            // passive effects or handled by other systems.
            _ => {}
        }

        // Dirty building discomfort: mild temperature drain for nearby cats.
        if structure.cleanliness < b.dirty_threshold {
            for (cat_pos, mut needs) in &mut cats {
                if cat_pos.manhattan_distance(&center) <= b.dirty_discomfort_radius {
                    needs.temperature = (needs.temperature
                        - dirty_temperature_drain * (1.0 - structure.cleanliness))
                        .max(0.0);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// decay_building_condition
// ---------------------------------------------------------------------------

/// Each tick, structural integrity and cleanliness decay separately.
///
/// Structural integrity only decays from harsh weather (storms, snow, heavy
/// rain). Fair-weather buildings don't deteriorate structurally.
///
/// Cleanliness decays in all weather, faster in bad conditions.
pub fn decay_building_condition(
    mut buildings: Query<&mut Structure>,
    weather: Res<WeatherState>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
) {
    let b = &constants.buildings;
    // Structural decay: very slow, only from harsh weather.
    let structural_decay = match weather.current {
        Weather::Storm => b.structural_decay_storm.per_tick(&time_scale),
        Weather::Snow => b.structural_decay_snow.per_tick(&time_scale),
        Weather::HeavyRain => b.structural_decay_heavy_rain.per_tick(&time_scale),
        _ => 0.0,
    };

    // Cleanliness decay: routine, from weather and use.
    let cleanliness_decay = match weather.current {
        Weather::HeavyRain | Weather::Storm => b.cleanliness_decay_storm.per_tick(&time_scale),
        Weather::Snow | Weather::Wind => b.cleanliness_decay_snow.per_tick(&time_scale),
        Weather::LightRain | Weather::Fog => b.cleanliness_decay_fog.per_tick(&time_scale),
        _ => b.cleanliness_decay_clear.per_tick(&time_scale),
    };

    for mut structure in &mut buildings {
        structure.condition = (structure.condition - structural_decay).max(0.0);
        structure.cleanliness = (structure.cleanliness - cleanliness_decay).max(0.0);
    }
}

// ---------------------------------------------------------------------------
// tidy_buildings
// ---------------------------------------------------------------------------

/// Cats that are idle or grooming near buildings passively restore cleanliness.
pub fn tidy_buildings(
    cats: Query<(&Position, &crate::ai::CurrentAction), Without<Dead>>,
    mut buildings: Query<(&Position, &mut Structure)>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
) {
    let b = &constants.buildings;
    let tidy_cleanliness_rate = b.tidy_cleanliness_rate.per_tick(&time_scale);
    for (cat_pos, action) in &cats {
        // 158: tidiness only ticks while a cat is genuinely loitering
        // — Idle or self-grooming. Allogrooming (`GroomOther`) is a
        // social interaction; it doesn't tidy the building.
        if !matches!(
            action.action,
            crate::ai::Action::Idle | crate::ai::Action::GroomSelf
        ) {
            continue;
        }
        for (building_pos, mut structure) in &mut buildings {
            let center = structure.center(building_pos);
            if cat_pos.manhattan_distance(&center) <= b.tidy_radius {
                activation.record(Feature::BuildingTidied);
                structure.cleanliness = (structure.cleanliness + tidy_cleanliness_rate).min(1.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// process_gates
// ---------------------------------------------------------------------------

/// After cats have moved, check gate state based on cat presence.
///
/// - A cat standing on a gate tile opens it.
/// - When no cat is on the gate, check cats one tile away (just walked through).
///   A diligent cat closes the gate behind them. A careless or tired cat leaves
///   it open. This creates the emergent chain: careless cat → open gate →
///   wildlife enters.
pub fn process_gates(
    mut gates: Query<(&Position, &mut GateState)>,
    cats: Query<(&Position, &Personality, &Needs), Without<Dead>>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let b = &constants.buildings;
    for (gate_pos, mut gate) in &mut gates {
        let cat_on_gate = cats.iter().any(|(pos, _, _)| pos == gate_pos);

        if cat_on_gate {
            if !gate.open {
                activation.record(Feature::GateProcessed);
            }
            gate.open = true;
        } else if gate.open {
            let mut best_diligence: Option<f32> = None;
            for (cat_pos, personality, needs) in &cats {
                if cat_pos.manhattan_distance(gate_pos) == 1 {
                    let effective = if needs.energy < b.gate_tired_energy_threshold {
                        personality.diligence * b.gate_tired_diligence_scale
                    } else {
                        personality.diligence
                    };
                    best_diligence =
                        Some(best_diligence.map_or(effective, |prev: f32| prev.max(effective)));
                }
            }

            if let Some(diligence) = best_diligence {
                if diligence > b.gate_close_diligence_threshold {
                    activation.record(Feature::GateProcessed);
                    gate.open = false;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Influence-map writers (ticket 006 — §5.6.3 producer landings)
// ---------------------------------------------------------------------------

/// Refresh the `ColonyLandmarks` resource from live `Structure`
/// entities. §L2.10.7 anchor-resolution path: cat self-state DSEs
/// (Eat / Cook / Farm / HerbcraftPrepare) request a single colony
/// landmark via `LandmarkAnchor::NearestKitchen` / `NearestStores` /
/// `NearestGarden`; the closure resolves them by reading this cache.
///
/// Picks the first functional instance encountered for each kind —
/// "single-instance" semantics matching today's solo-colony assumption.
/// Multi-instance support (nearest-of-many) lands when a colony grows
/// past one of any building.
pub fn update_colony_landmarks(
    buildings: Query<(&Structure, &Position), Without<ConstructionSite>>,
    mut landmarks: ResMut<crate::resources::ColonyLandmarks>,
) {
    *landmarks = crate::resources::ColonyLandmarks::empty();
    for (structure, anchor) in &buildings {
        if structure.effectiveness() <= 0.0 {
            continue;
        }
        let center = structure.center(anchor);
        match structure.kind {
            StructureType::Kitchen if landmarks.kitchen.is_none() => {
                landmarks.kitchen = Some(center);
            }
            StructureType::Stores if landmarks.stores.is_none() => {
                landmarks.stores = Some(center);
            }
            StructureType::Garden if landmarks.garden.is_none() => {
                landmarks.garden = Some(center);
            }
            _ => {}
        }
    }
}

/// Re-stamp `FoodLocationMap` from live `Stores` and `Kitchen`
/// `Structure` entities. §5.6.3 row #7 — sight × colony.
///
/// Each functional building (effectiveness > 0) paints a linear-falloff
/// disc of `food_location_sense_range` tiles, centered on the building's
/// computed center and weighted by effectiveness. Overlapping buildings
/// sum (clamped to 1.0).
///
/// Producer-only at landing — no DSE consumes this map yet (ticket 052
/// owns the consumer cutover). Behavior-neutral.
pub fn update_food_location_map(
    buildings: Query<(&Structure, &Position), Without<ConstructionSite>>,
    mut map: ResMut<crate::resources::FoodLocationMap>,
    constants: Res<SimConstants>,
) {
    let sense_range = constants.influence_maps.food_location_sense_range;
    map.clear();
    for (structure, anchor) in &buildings {
        if !matches!(structure.kind, StructureType::Stores | StructureType::Kitchen) {
            continue;
        }
        let eff = structure.effectiveness();
        if eff <= 0.0 {
            continue;
        }
        let center = structure.center(anchor);
        map.stamp(center.x, center.y, eff, sense_range);
    }
}

/// Re-stamp `GardenLocationMap` from live `Garden` `Structure`
/// entities. §5.6.3 row #10 — sight × colony.
///
/// Each functional garden paints a linear-falloff disc of
/// `garden_location_sense_range` tiles weighted by effectiveness.
/// Producer-only — no DSE consumes this map yet.
pub fn update_garden_location_map(
    buildings: Query<(&Structure, &Position), Without<ConstructionSite>>,
    mut map: ResMut<crate::resources::GardenLocationMap>,
    constants: Res<SimConstants>,
) {
    let sense_range = constants.influence_maps.garden_location_sense_range;
    map.clear();
    for (structure, anchor) in &buildings {
        if structure.kind != StructureType::Garden {
            continue;
        }
        let eff = structure.effectiveness();
        if eff <= 0.0 {
            continue;
        }
        let center = structure.center(anchor);
        map.stamp(center.x, center.y, eff, sense_range);
    }
}

/// Re-stamp `ConstructionSiteMap` from active `ConstructionSite`
/// entities and damaged `Structure` entities. §5.6.3 row #9 — sight ×
/// colony. Strength encodes urgency: `1 - progress` for in-progress
/// sites, `1 - condition` for damaged structures (only when condition
/// is below `damaged_threshold`, mirroring the §4 `HasDamagedBuilding`
/// marker predicate).
///
/// Producer-only — Build / Repair target ranking via
/// `SpatialConsideration` is owned by ticket 052.
pub fn update_construction_site_map(
    sites: Query<(&ConstructionSite, &Position)>,
    structures: Query<(&Structure, &Position), Without<ConstructionSite>>,
    mut map: ResMut<crate::resources::ConstructionSiteMap>,
    constants: Res<SimConstants>,
) {
    let cfg = &constants.influence_maps;
    let sense_range = cfg.construction_site_sense_range;
    map.clear();
    for (site, pos) in &sites {
        let urgency = (1.0 - site.progress).clamp(0.0, 1.0);
        if urgency <= 0.0 {
            continue;
        }
        map.stamp(pos.x, pos.y, urgency, sense_range);
    }
    for (structure, anchor) in &structures {
        if structure.condition >= cfg.damaged_threshold {
            continue;
        }
        let urgency = (1.0 - structure.condition).clamp(0.0, 1.0);
        if urgency <= 0.0 {
            continue;
        }
        let center = structure.center(anchor);
        map.stamp(center.x, center.y, urgency, sense_range);
    }
}

// ---------------------------------------------------------------------------
// update_colony_building_markers (ticket 168, extended by 169)
// ---------------------------------------------------------------------------

/// Authors colony-scoped building/food markers on the `ColonyState`
/// singleton (substrate spec §4.3 Inventory + TargetExistence rows).
/// Runs each FixedUpdate tick before `evaluate_and_plan`; the evaluator
/// reads these via `Has<MarkerN>` off the singleton (`colony_state_query`
/// in `goap.rs`) to populate `MarkerSnapshot`.
///
/// Markers authored:
/// - `HasFunctionalKitchen` — per `scan_colony_buildings`.
/// - `HasRawFoodInStores` — ≥1 raw-food item in any `StoredItems`.
/// - `HasStoredFood` — `FoodStores.is_empty()` is false.
/// - `HasConstructionSite` — ≥1 reachable `ConstructionSite` (ticket 169).
/// - `HasDamagedBuilding` — ≥1 `Structure` with condition <
///   `DispositionConstants::damaged_building_threshold` (ticket 169).
/// - `HasGarden` — ≥1 `Garden` `Structure` (ticket 171).
/// - `ColonyStoresChronicallyFull` — ticket 176; toggled when the
///   per-window count of `Feature::DepositRejected` divided by colony
///   cat-count exceeds `chronicity_threshold`. Drives the Build DSE
///   "we need more Stores" lift (default-zero weight in stage 4).
#[allow(clippy::too_many_arguments)]
pub fn update_colony_building_markers(
    mut commands: Commands,
    colony: Single<Entity, With<crate::components::markers::ColonyState>>,
    buildings: Query<(&Structure, Option<&ConstructionSite>)>,
    stored_items: Query<&crate::components::building::StoredItems>,
    items: Query<
        &crate::components::items::Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    cats: Query<&crate::components::physical::Health, Without<Dead>>,
    food: Res<FoodStores>,
    constants: Res<SimConstants>,
    activation: Option<Res<SystemActivation>>,
    time: Res<TimeState>,
    mut tracker: ResMut<crate::resources::stores_pressure::StoresPressureTracker>,
) {
    let d = &constants.disposition;
    let bldg_state = scan_colony_buildings(buildings.iter(), d.damaged_building_threshold);
    let has_raw_food_in_stores = stored_items.iter().any(|stored| {
        stored
            .items
            .iter()
            .copied()
            .any(|e| items.get(e).is_ok_and(|it| it.kind.is_food() && !it.modifiers.cooked))
    });
    let has_stored_food = !food.is_empty();

    // 176: chronicity tracking for `ColonyStoresChronicallyFull`. The
    // `SystemActivation::counts` map carries cumulative counts per
    // Feature; we periodically snapshot the `DepositRejected` total
    // and compute the per-window delta. Discrete snapshot
    // (every `chronicity_window_ticks` ticks) is simpler than a
    // sliding ringbuffer and produces a marker that flips at most
    // once per window, which is the right cadence for Build / Coordinator.
    let scoring = &constants.scoring;
    let cat_count = cats.iter().count().max(1) as f32;
    let current_rejections = activation
        .as_deref()
        .and_then(|sa| sa.counts.get(&Feature::DepositRejected).copied())
        .unwrap_or(0);
    let stores_chronically_full = if time
        .tick
        .saturating_sub(tracker.last_window_tick)
        >= scoring.chronicity_window_ticks
    {
        // Window boundary — compute the delta and flip the latched
        // verdict for the next window.
        let delta = current_rejections.saturating_sub(tracker.last_window_baseline);
        let per_cat = (delta as f32) / cat_count;
        tracker.last_window_baseline = current_rejections;
        tracker.last_window_tick = time.tick;
        tracker.latched_chronic = per_cat >= scoring.chronicity_threshold;
        tracker.latched_chronic
    } else {
        tracker.latched_chronic
    };

    let entity = *colony;
    let mut em = commands.entity(entity);
    if bldg_state.has_functional_kitchen {
        em.insert(crate::components::markers::HasFunctionalKitchen);
    } else {
        em.remove::<crate::components::markers::HasFunctionalKitchen>();
    }
    if has_raw_food_in_stores {
        em.insert(crate::components::markers::HasRawFoodInStores);
    } else {
        em.remove::<crate::components::markers::HasRawFoodInStores>();
    }
    if has_stored_food {
        em.insert(crate::components::markers::HasStoredFood);
    } else {
        em.remove::<crate::components::markers::HasStoredFood>();
    }
    if bldg_state.has_construction_site {
        em.insert(crate::components::markers::HasConstructionSite);
    } else {
        em.remove::<crate::components::markers::HasConstructionSite>();
    }
    if bldg_state.has_damaged_building {
        em.insert(crate::components::markers::HasDamagedBuilding);
    } else {
        em.remove::<crate::components::markers::HasDamagedBuilding>();
    }
    if bldg_state.has_garden {
        em.insert(crate::components::markers::HasGarden);
    } else {
        em.remove::<crate::components::markers::HasGarden>();
    }
    if stores_chronically_full {
        em.insert(crate::components::markers::ColonyStoresChronicallyFull);
    } else {
        em.remove::<crate::components::markers::ColonyStoresChronicallyFull>();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::building::Structure;
    use bevy_ecs::schedule::Schedule;

    fn test_time_scale() -> TimeScale {
        TimeScale::from_config(&SimConfig::default(), 16.6667)
    }

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(FoodStores::default());
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            speed: crate::resources::SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(test_time_scale());
        world.insert_resource(WeatherState::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        world
    }

    #[test]
    fn den_provides_temperature_and_safety_within_range() {
        let mut world = test_world();

        // Den (3×3) at anchor (5, 5), center = (6, 6). Radius = 4.
        world.spawn((Structure::new(StructureType::Den), Position::new(5, 5)));

        // Cat within range: distance 2 from center (6,6)
        let near_cat = world.spawn((Position::new(7, 7), Needs::default())).id();

        // Cat outside range: distance 6 from center (6,6) — well beyond radius 4
        let far_cat = world.spawn((Position::new(12, 6), Needs::default())).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_building_effects);
        schedule.run(&mut world);

        let near_needs = world.get::<Needs>(near_cat).unwrap();
        assert!(
            near_needs.temperature > 0.9,
            "near cat should get warmth bonus"
        );
        assert!(
            near_needs.safety > 1.0 - f32::EPSILON,
            "near cat should get safety bonus"
        );

        let far_needs = world.get::<Needs>(far_cat).unwrap();
        assert!(
            (far_needs.temperature - 0.9).abs() < 1e-6,
            "far cat should not get warmth bonus"
        );
    }

    #[test]
    fn hearth_provides_social_bonus() {
        let mut world = test_world();

        world.spawn((Structure::new(StructureType::Hearth), Position::new(5, 5)));

        let cat = world
            .spawn((Position::new(5, 7), Needs::default())) // distance 2
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_building_effects);
        schedule.run(&mut world);

        let needs = world.get::<Needs>(cat).unwrap();
        assert!(
            needs.social > 0.6,
            "cat near hearth should get social bonus (got {})",
            needs.social
        );
    }

    #[test]
    fn stores_halves_spoilage_multiplier() {
        let mut world = test_world();

        world.spawn((Structure::new(StructureType::Stores), Position::new(5, 5)));

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_building_effects);
        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert!(
            (food.spoilage_multiplier - 0.5).abs() < 1e-6,
            "Stores should halve spoilage multiplier"
        );
    }

    #[test]
    fn stores_no_effect_when_non_functional() {
        let mut world = test_world();

        world.spawn((
            Structure {
                kind: StructureType::Stores,
                condition: 0.1, // below 0.2 threshold
                cleanliness: 1.0,
                size: StructureType::Stores.default_size(),
            },
            Position::new(5, 5),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_building_effects);
        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert!(
            (food.spoilage_multiplier - 1.0).abs() < 1e-6,
            "non-functional Stores should not affect spoilage"
        );
    }

    #[test]
    fn no_structural_decay_in_clear_weather() {
        let mut world = World::new();
        world.insert_resource(WeatherState {
            current: Weather::Clear,
            ticks_until_change: 50,
        });
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(test_time_scale());

        let building = world.spawn(Structure::new(StructureType::Den)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(decay_building_condition);
        schedule.run(&mut world);

        let s = world.get::<Structure>(building).unwrap();
        assert_eq!(
            s.condition, 1.0,
            "clear weather should not decay structural condition"
        );
        assert!(
            s.cleanliness < 1.0,
            "cleanliness should decay even in clear weather"
        );
    }

    #[test]
    fn structural_decay_in_storm() {
        let mut world = World::new();
        world.insert_resource(WeatherState {
            current: Weather::Storm,
            ticks_until_change: 50,
        });
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(test_time_scale());

        let building = world.spawn(Structure::new(StructureType::Den)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(decay_building_condition);
        schedule.run(&mut world);

        let s = world.get::<Structure>(building).unwrap();
        let expected_condition = 1.0 - 0.00003;
        assert!(
            (s.condition - expected_condition).abs() < 1e-6,
            "storm should cause structural decay (expected {expected_condition}, got {})",
            s.condition
        );
        let expected_cleanliness = 1.0 - 0.0002;
        assert!(
            (s.cleanliness - expected_cleanliness).abs() < 1e-6,
            "storm should decay cleanliness faster (expected {expected_cleanliness}, got {})",
            s.cleanliness
        );
    }

    #[test]
    fn condition_does_not_go_negative() {
        let mut world = World::new();
        world.insert_resource(WeatherState {
            current: Weather::Storm,
            ticks_until_change: 50,
        });
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(test_time_scale());

        let building = world
            .spawn(Structure {
                kind: StructureType::Den,
                condition: 0.00001,   // below storm structural_decay (0.00003)
                cleanliness: 0.00001, // below storm cleanliness_decay (0.0002)
                size: StructureType::Den.default_size(),
            })
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(decay_building_condition);
        schedule.run(&mut world);

        let s = world.get::<Structure>(building).unwrap();
        assert_eq!(s.condition, 0.0, "condition should not go negative");
        assert_eq!(s.cleanliness, 0.0, "cleanliness should not go negative");
    }

    #[test]
    fn spoilage_multiplier_resets_each_tick() {
        let mut world = test_world();
        // Set multiplier to something non-default
        world.resource_mut::<FoodStores>().spoilage_multiplier = 0.5;

        // No Stores building
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_building_effects);
        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert!(
            (food.spoilage_multiplier - 1.0).abs() < 1e-6,
            "multiplier should reset to 1.0 when no Stores exists"
        );
    }

    // --- scan_colony_buildings ---

    #[test]
    fn empty_buildings_all_false() {
        let state = scan_colony_buildings(std::iter::empty(), 0.4);
        assert!(!state.has_construction_site);
        assert!(!state.has_damaged_building);
        assert!(!state.has_garden);
        assert!(!state.has_functional_kitchen);
    }

    #[test]
    fn construction_site_detected() {
        let site = ConstructionSite::new(StructureType::Den);
        let structure = Structure::new(StructureType::Den);
        let buildings: Vec<(&Structure, Option<&ConstructionSite>)> =
            vec![(&structure, Some(&site))];
        let state = scan_colony_buildings(buildings.into_iter(), 0.4);
        assert!(state.has_construction_site);
        assert!(!state.has_garden);
    }

    #[test]
    fn garden_detected() {
        let structure = Structure::new(StructureType::Garden);
        let buildings: Vec<(&Structure, Option<&ConstructionSite>)> = vec![(&structure, None)];
        let state = scan_colony_buildings(buildings.into_iter(), 0.4);
        assert!(state.has_garden);
        assert!(!state.has_functional_kitchen);
    }

    #[test]
    fn functional_kitchen_detected() {
        let mut kitchen = Structure::new(StructureType::Kitchen);
        kitchen.condition = 1.0; // effectiveness > 0 when condition > 0
        let buildings: Vec<(&Structure, Option<&ConstructionSite>)> = vec![(&kitchen, None)];
        let state = scan_colony_buildings(buildings.into_iter(), 0.4);
        assert!(state.has_functional_kitchen);
    }

    #[test]
    fn kitchen_under_construction_not_functional() {
        let kitchen = Structure::new(StructureType::Kitchen);
        let site = ConstructionSite::new(StructureType::Kitchen);
        let buildings: Vec<(&Structure, Option<&ConstructionSite>)> = vec![(&kitchen, Some(&site))];
        let state = scan_colony_buildings(buildings.into_iter(), 0.4);
        assert!(!state.has_functional_kitchen);
        assert!(state.has_construction_site);
    }

    #[test]
    fn damaged_building_below_threshold() {
        let mut structure = Structure::new(StructureType::Den);
        structure.condition = 0.3; // below 0.4 threshold
        let buildings: Vec<(&Structure, Option<&ConstructionSite>)> = vec![(&structure, None)];
        let state = scan_colony_buildings(buildings.into_iter(), 0.4);
        assert!(state.has_damaged_building);
    }

    #[test]
    fn building_above_threshold_not_damaged() {
        let structure = Structure::new(StructureType::Den);
        // Default condition is 1.0.
        let buildings: Vec<(&Structure, Option<&ConstructionSite>)> = vec![(&structure, None)];
        let state = scan_colony_buildings(buildings.into_iter(), 0.4);
        assert!(!state.has_damaged_building);
    }

    // --- update_colony_building_markers (ticket 169) ---
    //
    // Tick-system tests asserting `HasConstructionSite` and
    // `HasDamagedBuilding` are inserted/removed on the `ColonyState`
    // singleton. Modeled on `growth.rs::update_parent_markers` tests
    // (bare `Schedule` + `World`, no full `App`).

    use crate::components::markers;

    fn setup_colony_markers() -> (World, Schedule) {
        let mut world = test_world();
        world.spawn(markers::ColonyState);
        // 176: chronicity tracker resource — default-zero so the
        // marker stays cleared in tests that don't exercise it.
        world.insert_resource(
            crate::resources::stores_pressure::StoresPressureTracker::default(),
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(update_colony_building_markers);
        (world, schedule)
    }

    fn colony_entity(world: &mut World) -> Entity {
        world
            .query_filtered::<Entity, With<markers::ColonyState>>()
            .single(world)
            .expect("ColonyState singleton must exist")
    }

    #[test]
    fn colony_marker_set_when_construction_site_exists() {
        let (mut world, mut schedule) = setup_colony_markers();
        world.spawn((
            Structure::new(StructureType::Den),
            ConstructionSite::new(StructureType::Den),
        ));
        schedule.run(&mut world);
        let colony = colony_entity(&mut world);
        assert!(world.entity(colony).contains::<markers::HasConstructionSite>());
    }

    #[test]
    fn colony_marker_cleared_when_no_construction_site() {
        let (mut world, mut schedule) = setup_colony_markers();
        // Plain structure, no ConstructionSite.
        let mut structure = Structure::new(StructureType::Den);
        structure.condition = 1.0;
        world.spawn(structure);
        schedule.run(&mut world);
        let colony = colony_entity(&mut world);
        assert!(!world.entity(colony).contains::<markers::HasConstructionSite>());
    }

    #[test]
    fn colony_damage_marker_set_when_below_threshold() {
        let (mut world, mut schedule) = setup_colony_markers();
        let mut structure = Structure::new(StructureType::Den);
        structure.condition = 0.3; // below default 0.4 threshold
        world.spawn(structure);
        schedule.run(&mut world);
        let colony = colony_entity(&mut world);
        assert!(world.entity(colony).contains::<markers::HasDamagedBuilding>());
    }

    #[test]
    fn colony_damage_marker_cleared_when_above_threshold() {
        let (mut world, mut schedule) = setup_colony_markers();
        let mut structure = Structure::new(StructureType::Den);
        structure.condition = 0.9; // above 0.4 threshold
        world.spawn(structure);
        schedule.run(&mut world);
        let colony = colony_entity(&mut world);
        assert!(!world.entity(colony).contains::<markers::HasDamagedBuilding>());
    }

    #[test]
    fn colony_damage_marker_skipped_for_in_progress_construction() {
        // Buildings with a `ConstructionSite` must NOT count as damaged
        // even if their condition is low — `scan_colony_buildings`
        // routes those into `has_construction_site` only.
        let (mut world, mut schedule) = setup_colony_markers();
        let mut structure = Structure::new(StructureType::Den);
        structure.condition = 0.1;
        world.spawn((structure, ConstructionSite::new(StructureType::Den)));
        schedule.run(&mut world);
        let colony = colony_entity(&mut world);
        assert!(world.entity(colony).contains::<markers::HasConstructionSite>());
        assert!(!world.entity(colony).contains::<markers::HasDamagedBuilding>());
    }

    #[test]
    fn colony_garden_marker_set_when_garden_exists() {
        let (mut world, mut schedule) = setup_colony_markers();
        world.spawn(Structure::new(StructureType::Garden));
        schedule.run(&mut world);
        let colony = colony_entity(&mut world);
        assert!(world.entity(colony).contains::<markers::HasGarden>());
    }

    #[test]
    fn colony_garden_marker_cleared_when_no_garden() {
        let (mut world, mut schedule) = setup_colony_markers();
        world.spawn(Structure::new(StructureType::Den));
        schedule.run(&mut world);
        let colony = colony_entity(&mut world);
        assert!(!world.entity(colony).contains::<markers::HasGarden>());
    }
}
