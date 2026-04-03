use bevy_ecs::prelude::*;

use crate::components::building::{ConstructionSite, GateState, Structure, StructureType};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::resources::food::FoodStores;
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
) {
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

        match structure.kind {
            StructureType::Den => {
                for (cat_pos, mut needs) in &mut cats {
                    if cat_pos.manhattan_distance(building_pos) <= 2 {
                        needs.warmth = (needs.warmth + 0.01 * eff).min(1.0);
                        needs.safety = (needs.safety + 0.005 * eff).min(1.0);
                    }
                }
            }
            StructureType::Hearth => {
                for (cat_pos, mut needs) in &mut cats {
                    if cat_pos.manhattan_distance(building_pos) <= 3 {
                        needs.social = (needs.social + 0.01 * eff).min(1.0);
                        if is_cold {
                            needs.warmth = (needs.warmth + 0.01 * eff).min(1.0);
                        }
                    }
                }
            }
            StructureType::Stores => {
                food.spoilage_multiplier = 0.5;
            }
            // Workshop, Watchtower, WardPost, Wall, Gate, Garden:
            // passive effects or handled by other systems.
            _ => {}
        }

        // Dirty building discomfort: mild warmth drain for nearby cats.
        if structure.cleanliness < 0.3 {
            for (cat_pos, mut needs) in &mut cats {
                if cat_pos.manhattan_distance(building_pos) <= 2 {
                    needs.warmth = (needs.warmth
                        - 0.003 * (1.0 - structure.cleanliness))
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
) {
    // Structural decay: very slow, only from harsh weather.
    let structural_decay = match weather.current {
        Weather::Storm => 0.0003,
        Weather::Snow => 0.0002,
        Weather::HeavyRain => 0.0001,
        _ => 0.0,
    };

    // Cleanliness decay: routine, from weather and use.
    let cleanliness_decay = match weather.current {
        Weather::HeavyRain | Weather::Storm => 0.002,
        Weather::Snow | Weather::Wind => 0.0015,
        Weather::LightRain | Weather::Fog => 0.001,
        _ => 0.0008,
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
    cats: Query<
        (&Position, &crate::ai::CurrentAction),
        Without<Dead>,
    >,
    mut buildings: Query<(&Position, &mut Structure)>,
) {
    for (cat_pos, action) in &cats {
        if !matches!(
            action.action,
            crate::ai::Action::Idle | crate::ai::Action::Groom
        ) {
            continue;
        }
        for (building_pos, mut structure) in &mut buildings {
            if cat_pos.manhattan_distance(building_pos) <= 2 {
                structure.cleanliness = (structure.cleanliness + 0.005).min(1.0);
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
) {
    for (gate_pos, mut gate) in &mut gates {
        let cat_on_gate = cats.iter().any(|(pos, _, _)| pos == gate_pos);

        if cat_on_gate {
            gate.open = true;
        } else if gate.open {
            // No cat on gate — did someone just walk through?
            // Check adjacent cats (distance 1 = just departed this tick).
            let mut best_diligence: Option<f32> = None;
            for (cat_pos, personality, needs) in &cats {
                if cat_pos.manhattan_distance(gate_pos) == 1 {
                    // Tired cats act less diligent.
                    let effective = if needs.energy < 0.3 {
                        personality.diligence * 0.6
                    } else {
                        personality.diligence
                    };
                    best_diligence = Some(
                        best_diligence.map_or(effective, |prev: f32| prev.max(effective)),
                    );
                }
            }

            if let Some(diligence) = best_diligence {
                if diligence > 0.5 {
                    gate.open = false;
                }
                // Otherwise: careless cat left it open.
            }
            // No adjacent cat: gate stays in current state (open).
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;
    use crate::components::building::Structure;

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(FoodStores::default());
        world.insert_resource(TimeState { tick: 0, paused: false, speed: crate::resources::SimSpeed::Normal });
        world.insert_resource(SimConfig::default());
        world.insert_resource(WeatherState::default());
        world
    }

    #[test]
    fn den_provides_warmth_and_safety_within_range() {
        let mut world = test_world();

        // Den at (5, 5)
        world.spawn((
            Structure::new(StructureType::Den),
            Position::new(5, 5),
        ));

        // Cat within range (distance 2)
        let near_cat = world
            .spawn((Position::new(6, 6), Needs::default()))
            .id();

        // Cat outside range (distance 4)
        let far_cat = world
            .spawn((Position::new(9, 5), Needs::default()))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_building_effects);
        schedule.run(&mut world);

        let near_needs = world.get::<Needs>(near_cat).unwrap();
        // Default warmth is 0.9, Den adds 0.01 → 0.91
        assert!(near_needs.warmth > 0.9, "near cat should get warmth bonus");
        assert!(near_needs.safety > 1.0 - f32::EPSILON, "near cat should get safety bonus");

        let far_needs = world.get::<Needs>(far_cat).unwrap();
        assert!(
            (far_needs.warmth - 0.9).abs() < 1e-6,
            "far cat should not get warmth bonus"
        );
    }

    #[test]
    fn hearth_provides_social_bonus() {
        let mut world = test_world();

        world.spawn((
            Structure::new(StructureType::Hearth),
            Position::new(5, 5),
        ));

        let cat = world
            .spawn((Position::new(5, 7), Needs::default())) // distance 2
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_building_effects);
        schedule.run(&mut world);

        let needs = world.get::<Needs>(cat).unwrap();
        assert!(needs.social > 0.6, "cat near hearth should get social bonus (got {})", needs.social);
    }

    #[test]
    fn stores_halves_spoilage_multiplier() {
        let mut world = test_world();

        world.spawn((
            Structure::new(StructureType::Stores),
            Position::new(5, 5),
        ));

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
                size: (2, 2),
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

        let building = world
            .spawn(Structure::new(StructureType::Den))
            .id();

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

        let building = world
            .spawn(Structure::new(StructureType::Den))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(decay_building_condition);
        schedule.run(&mut world);

        let s = world.get::<Structure>(building).unwrap();
        let expected_condition = 1.0 - 0.0003;
        assert!(
            (s.condition - expected_condition).abs() < 1e-6,
            "storm should cause structural decay (expected {expected_condition}, got {})",
            s.condition
        );
        let expected_cleanliness = 1.0 - 0.002;
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

        let building = world
            .spawn(Structure {
                kind: StructureType::Den,
                condition: 0.0001,
                cleanliness: 0.0001,
                size: (2, 2),
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
