use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::physical::{Health, Position};
use crate::components::prey::{PreyAnimal, PreySpecies};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try up to 50 random tiles to find one whose terrain is in `species.habitat()`.
/// Returns `None` if no suitable tile is found.
pub fn find_habitat_tile(species: PreySpecies, map: &TileMap, rng: &mut SimRng) -> Option<Position> {
    let habitat = species.habitat();
    for _ in 0..50 {
        let x = rng.rng.random_range(0..map.width);
        let y = rng.rng.random_range(0..map.height);
        if habitat.contains(&map.get(x, y).terrain) {
            return Some(Position::new(x, y));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// prey_population system
// ---------------------------------------------------------------------------

/// Breed prey when below population cap; log overcrowding warnings.
pub fn prey_population(
    mut commands: Commands,
    query: Query<&PreyAnimal>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
) {
    // Count living prey per species.
    let mut counts: HashMap<PreySpecies, usize> = HashMap::new();
    for animal in &query {
        *counts.entry(animal.species).or_insert(0) += 1;
    }

    let all_species = [
        PreySpecies::Mouse,
        PreySpecies::Rat,
        PreySpecies::Fish,
        PreySpecies::Bird,
    ];

    for species in all_species {
        let pop = *counts.get(&species).unwrap_or(&0);
        let cap = species.population_cap();

        let density_pressure = 1.0 - (pop as f32 / cap as f32);

        if density_pressure <= 0.0 {
            // At or above cap — log rarely (~0.1% chance per tick).
            if rng.rng.random::<f32>() < 0.001 {
                log.push(
                    time.tick,
                    format!("The {} have overrun their territory.", species.name()),
                    NarrativeTier::Micro,
                );
            }
            continue;
        }

        if density_pressure < 0.2 {
            // Getting crowded — log occasionally (~0.2% chance per tick).
            if rng.rng.random::<f32>() < 0.002 {
                log.push(
                    time.tick,
                    format!("The {} are growing restless.", species.name()),
                    NarrativeTier::Micro,
                );
            }
        }

        // Attempt to breed. food_availability is hardcoded to 1.0 for now.
        let breed_chance = species.breed_rate() * density_pressure;
        if rng.rng.random::<f32>() < breed_chance {
            if let Some(pos) = find_habitat_tile(species, &map, &mut rng) {
                commands.spawn((PreyAnimal::new(species), Health::default(), pos));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// prey_hunger system
// ---------------------------------------------------------------------------

/// Advance hunger for all prey; despawn any that starve.
pub fn prey_hunger(
    mut commands: Commands,
    mut query: Query<(Entity, &mut PreyAnimal, &mut Health)>,
) {
    // Count population per species first (immutable pass) to avoid a conflicting
    // second Query over PreyAnimal.
    let mut counts: HashMap<PreySpecies, usize> = HashMap::new();
    for (_, animal, _) in query.iter() {
        *counts.entry(animal.species).or_insert(0) += 1;
    }

    for (entity, mut animal, mut health) in &mut query {
        let pop = *counts.get(&animal.species).unwrap_or(&0);
        let cap = animal.species.population_cap();

        // Base hunger increase.
        animal.hunger += 0.002;

        // Extra overcrowding hunger when above 80% of cap.
        if pop as f32 > cap as f32 * 0.8 {
            animal.hunger += 0.001;
        }

        // Simplified food access: prey always finds some food.
        animal.hunger -= 0.003;
        animal.hunger = animal.hunger.clamp(0.0, 1.0);

        // Starvation drains health; despawn at zero health.
        if animal.hunger > 0.9 {
            health.current -= 0.01;
        }

        if health.current <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// spawn_initial_prey (world-gen helper, not a system)
// ---------------------------------------------------------------------------

/// Spawn the initial prey population during world generation.
/// Called from `build_new_world`, not registered as a system.
///
/// Follows the same pattern as `spawn_initial_wildlife`: borrows resources
/// from the world, collects spawns, then spawns outside the borrow.
pub fn spawn_initial_prey(world: &mut World) {
    let all_species = [
        PreySpecies::Mouse,
        PreySpecies::Rat,
        PreySpecies::Fish,
        PreySpecies::Bird,
    ];

    // Snapshot terrain so we can release the map borrow before spawning.
    let (map_width, map_height, terrain_snapshot): (i32, i32, Vec<Terrain>) = {
        let map = world.resource::<TileMap>();
        let snapshot = (0..map.height)
            .flat_map(|y| (0..map.width).map(move |x| (x, y)))
            .map(|(x, y)| map.get(x, y).terrain)
            .collect();
        (map.width, map.height, snapshot)
    };

    let mut spawns: Vec<(PreySpecies, Position)> = Vec::new();

    {
        let rng = &mut world.resource_mut::<SimRng>().rng;

        for species in all_species {
            let habitat = species.habitat();
            let count = species.population_cap() / 3;
            let mut spawned = 0;
            let mut attempts = 0;
            while spawned < count && attempts < count * 50 {
                attempts += 1;
                let x: i32 = rng.random_range(0..map_width);
                let y: i32 = rng.random_range(0..map_height);
                let terrain = terrain_snapshot[(y * map_width + x) as usize];
                if habitat.contains(&terrain) {
                    spawns.push((species, Position::new(x, y)));
                    spawned += 1;
                }
            }
        }
    }

    for (species, pos) in spawns {
        world.spawn((PreyAnimal::new(species), Health::default(), pos));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;
    use crate::resources::map::Terrain;

    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        let map = TileMap::new(20, 20, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            speed: crate::resources::SimSpeed::Normal,
        });
        let mut schedule = Schedule::default();
        schedule.add_systems((prey_population, prey_hunger).chain());
        (world, schedule)
    }

    #[test]
    fn prey_breed_when_below_cap() {
        let (mut world, mut schedule) = setup();

        // Spawn 5 mice (cap=30).
        for i in 0..5i32 {
            world.spawn((PreyAnimal::new(PreySpecies::Mouse), Health::default(), Position::new(i, 0)));
        }

        // Run enough ticks to make breeding near-certain.
        // Mouse breed_rate=0.003, density_pressure≈0.83 → breed_chance≈0.0025/tick.
        // P(≥1 breed in 2000 ticks) ≈ 99.3%, so 2000 is a safe threshold.
        for _ in 0..2000 {
            schedule.run(&mut world);
        }

        let count = world
            .query::<&PreyAnimal>()
            .iter(&world)
            .filter(|a| a.species == PreySpecies::Mouse)
            .count();

        assert!(count > 5, "mice should have bred after 2000 ticks, got {count}");
    }

    #[test]
    fn prey_do_not_exceed_cap() {
        let (mut world, mut schedule) = setup();

        // Spawn 30 mice — at cap.
        for i in 0..30i32 {
            world.spawn((
                PreyAnimal::new(PreySpecies::Mouse),
                Health::default(),
                Position::new(i % 20, i / 20),
            ));
        }

        // Run 100 ticks.
        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let count = world
            .query::<&PreyAnimal>()
            .iter(&world)
            .filter(|a| a.species == PreySpecies::Mouse)
            .count();

        assert!(
            count <= 30,
            "mice at cap should not exceed 30, got {count}"
        );
    }
}
