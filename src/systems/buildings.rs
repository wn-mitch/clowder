use bevy_ecs::prelude::*;

use crate::components::building::{ConstructionSite, GateState, Structure, StructureType};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::resources::food::FoodStores;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{Season, SimConfig, TimeState};
use crate::resources::weather::{Weather, WeatherState};

// ---------------------------------------------------------------------------
// apply_building_effects
// ---------------------------------------------------------------------------

/// Each tick, completed buildings provide passive bonuses to nearby cats.
///
/// Runs after `detect_threats` and before `decay_needs` so that building
/// bonuses are applied before needs decay subtracts from them.
pub fn apply_building_effects(
    buildings: Query<(&Structure, &Position), Without<ConstructionSite>>,
    mut cats: Query<(&Position, &mut Needs), Without<Dead>>,
    mut food: ResMut<FoodStores>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    constants: Res<SimConstants>,
) {
    let b = &constants.buildings;
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
                        needs.temperature = (needs.temperature + b.den_temperature_bonus * eff).min(1.0);
                        needs.safety = (needs.safety + b.den_safety_bonus * eff).min(1.0);
                    }
                }
            }
            StructureType::Hearth => {
                for (cat_pos, mut needs) in &mut cats {
                    if cat_pos.manhattan_distance(&center) <= b.hearth_effect_radius {
                        needs.social = (needs.social + b.hearth_social_bonus * eff).min(1.0);
                        if is_cold {
                            needs.temperature =
                                (needs.temperature + b.hearth_temperature_bonus_cold * eff).min(1.0);
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
                        - b.dirty_temperature_drain * (1.0 - structure.cleanliness))
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
) {
    let b = &constants.buildings;
    // Structural decay: very slow, only from harsh weather.
    let structural_decay = match weather.current {
        Weather::Storm => b.structural_decay_storm,
        Weather::Snow => b.structural_decay_snow,
        Weather::HeavyRain => b.structural_decay_heavy_rain,
        _ => 0.0,
    };

    // Cleanliness decay: routine, from weather and use.
    let cleanliness_decay = match weather.current {
        Weather::HeavyRain | Weather::Storm => b.cleanliness_decay_storm,
        Weather::Snow | Weather::Wind => b.cleanliness_decay_snow,
        Weather::LightRain | Weather::Fog => b.cleanliness_decay_fog,
        _ => b.cleanliness_decay_clear,
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
    mut activation: ResMut<SystemActivation>,
) {
    let b = &constants.buildings;
    for (cat_pos, action) in &cats {
        if !matches!(
            action.action,
            crate::ai::Action::Idle | crate::ai::Action::Groom
        ) {
            continue;
        }
        for (building_pos, mut structure) in &mut buildings {
            let center = structure.center(building_pos);
            if cat_pos.manhattan_distance(&center) <= b.tidy_radius {
                activation.record(Feature::BuildingTidied);
                structure.cleanliness = (structure.cleanliness + b.tidy_cleanliness_rate).min(1.0);
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::building::Structure;
    use bevy_ecs::schedule::Schedule;

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(FoodStores::default());
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            speed: crate::resources::SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
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
        assert!(near_needs.temperature > 0.9, "near cat should get warmth bonus");
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
}
