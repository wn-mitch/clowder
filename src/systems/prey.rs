use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::items::Item;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::prey::{PreyAiState, PreyAnimal, PreySpecies};
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
// prey_ai system
// ---------------------------------------------------------------------------

/// Advance the AI state machine for all living prey animals.
///
/// States: Idle → Grazing (random wander), Fleeing (run from threat).
/// Movement cadence and terrain checks mirror the wildlife patrol pattern.
pub fn prey_ai(
    mut query: Query<(&mut PreyAnimal, &mut Position), Without<Dead>>,
    positions: Query<&Position, Without<PreyAnimal>>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
) {
    for (mut animal, mut pos) in &mut query {
        match animal.ai_state {
            PreyAiState::Idle => {
                // 5% chance per tick to start grazing in a random direction.
                if rng.rng.random::<f32>() < 0.05 {
                    let dx = rng.rng.random_range(-1i32..=1);
                    let dy = rng.rng.random_range(-1i32..=1);
                    if dx != 0 || dy != 0 {
                        animal.ai_state = PreyAiState::Grazing { dx, dy, ticks: 0 };
                    }
                }
            }

            PreyAiState::Grazing { mut dx, mut dy, ticks } => {
                let new_ticks = ticks + 1;

                // 10% chance to jitter direction.
                if rng.rng.random::<f32>() < 0.1 {
                    let jdx = rng.rng.random_range(-1i32..=1);
                    let jdy = rng.rng.random_range(-1i32..=1);
                    if jdx != 0 || jdy != 0 {
                        dx = jdx;
                        dy = jdy;
                    }
                }

                // Move 1 tile every 30 ticks.
                if new_ticks % 30 == 0 {
                    let nx = pos.x + dx;
                    let ny = pos.y + dy;
                    let habitat = animal.species.habitat();

                    if map.in_bounds(nx, ny) && habitat.contains(&map.get(nx, ny).terrain) {
                        pos.x = nx;
                        pos.y = ny;
                    } else {
                        // Reverse direction.
                        dx = -dx;
                        dy = -dy;
                        let rx = pos.x + dx;
                        let ry = pos.y + dy;
                        if map.in_bounds(rx, ry) && habitat.contains(&map.get(rx, ry).terrain) {
                            pos.x = rx;
                            pos.y = ry;
                        }
                    }
                }

                if new_ticks >= 200 {
                    animal.ai_state = PreyAiState::Idle;
                } else {
                    animal.ai_state = PreyAiState::Grazing { dx, dy, ticks: new_ticks };
                }
            }

            PreyAiState::Fleeing { from, ticks } => {
                let new_ticks = ticks + 1;

                // Check if the threat still exists and is nearby.
                let threat_pos = positions.get(from).ok();
                let should_stop = new_ticks >= 150
                    || threat_pos.is_none()
                    || threat_pos
                        .map(|tp| pos.manhattan_distance(tp) > 10)
                        .unwrap_or(true);

                if should_stop {
                    animal.ai_state = PreyAiState::Idle;
                } else {
                    let tp = threat_pos.unwrap();
                    // Flee: move in the opposite direction of the threat.
                    let dx = (pos.x - tp.x).signum();
                    let dy = (pos.y - tp.y).signum();

                    // Try diagonal, then cardinals.
                    let candidates = [
                        (pos.x + dx, pos.y + dy),
                        (pos.x + dx, pos.y),
                        (pos.x, pos.y + dy),
                    ];

                    for (nx, ny) in candidates {
                        if map.in_bounds(nx, ny) && map.get(nx, ny).terrain.is_passable() {
                            pos.x = nx;
                            pos.y = ny;
                            break;
                        }
                    }

                    animal.ai_state = PreyAiState::Fleeing { from, ticks: new_ticks };
                }
            }
        }
    }
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
///
/// Mice and rats that wander within 2 tiles of an unguarded Stores building eat
/// from the stores instead of foraging in the wild — emergent pest pressure
/// from population density. A cat within 4 tiles of the stores deters pests.
pub fn prey_hunger(
    mut commands: Commands,
    mut query: Query<(Entity, &mut PreyAnimal, &mut Health, &Position), Without<Structure>>,
    mut stores_query: Query<(Entity, &mut Structure, &Position, &mut StoredItems), Without<PreyAnimal>>,
    cat_positions: Query<&Position, (With<Needs>, Without<Dead>, Without<PreyAnimal>)>,
    items_query: Query<&Item>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
) {
    // Count population per species first (immutable pass) to avoid a conflicting
    // second Query over PreyAnimal.
    let mut counts: HashMap<PreySpecies, usize> = HashMap::new();
    for (_, animal, _, _) in query.iter() {
        *counts.entry(animal.species).or_insert(0) += 1;
    }

    // Snapshot store positions and whether a cat is guarding each store.
    let store_positions: Vec<(Entity, Position, bool)> = stores_query
        .iter()
        .filter(|(_, s, _, _)| s.kind == StructureType::Stores)
        .map(|(e, _, p, _)| {
            let guarded = cat_positions
                .iter()
                .any(|cp| cp.manhattan_distance(p) <= 4);
            (e, *p, guarded)
        })
        .collect();

    for (entity, mut animal, mut health, pos) in &mut query {
        let pop = *counts.get(&animal.species).unwrap_or(&0);
        let cap = animal.species.population_cap();

        // Base hunger increase.
        animal.hunger += 0.0002;

        // Extra overcrowding hunger when above 80% of cap.
        if pop as f32 > cap as f32 * 0.8 {
            animal.hunger += 0.0001;
        }

        // Mice and rats near stores raid them; all other prey forage in the wild.
        // Only 5% chance per tick — not every nearby pest eats every tick.
        let mut ate_from_stores = false;
        if matches!(animal.species, PreySpecies::Mouse | PreySpecies::Rat)
            && rng.rng.random::<f32>() < 0.05
        {
            for &(store_entity, store_pos, guarded) in &store_positions {
                if guarded || pos.manhattan_distance(&store_pos) > 2 {
                    continue;
                }
                if let Ok((_, mut structure, _, mut stored)) =
                    stores_query.get_mut(store_entity)
                {
                    let food_entity = stored
                        .items
                        .iter()
                        .copied()
                        .find(|&e| items_query.get(e).is_ok_and(|i| i.kind.is_food()));
                    if let Some(food_entity) = food_entity {
                        stored.remove(food_entity);
                        commands.entity(food_entity).despawn();
                        animal.hunger = (animal.hunger - 0.015).max(0.0);
                        structure.cleanliness = (structure.cleanliness - 0.001).max(0.0);
                        ate_from_stores = true;

                        if rng.rng.random::<f32>() < 0.02 {
                            log.push(
                                time.tick,
                                format!(
                                    "A {} has gotten into the stores!",
                                    animal.species.name()
                                ),
                                NarrativeTier::Action,
                            );
                        }
                        break;
                    }
                }
            }
        }

        if !ate_from_stores {
            animal.hunger -= 0.0003;
        }
        animal.hunger = animal.hunger.clamp(0.0, 1.0);

        // Starvation drains health; despawn at zero health.
        if animal.hunger > 0.9 {
            health.current -= 0.001;
        }

        if health.current <= 0.0 {
            // Narrate prey starvation (~10% of deaths).
            if rng.rng.random::<f32>() < 0.1 {
                let species_name = animal.species.name();
                log.push(
                    time.tick,
                    format!("A {species_name} collapses from hunger."),
                    NarrativeTier::Micro,
                );
            }
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
            let count = species.population_cap() * 3 / 4;
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

        // Spawn 5 mice (cap=80).
        for i in 0..5i32 {
            world.spawn((PreyAnimal::new(PreySpecies::Mouse), Health::default(), Position::new(i, 0)));
        }

        // Run enough ticks to make breeding near-certain.
        // Mouse breed_rate=0.003, density_pressure≈0.94 → breed_chance≈0.0028/tick.
        // P(≥1 breed in 2000 ticks) ≈ 99.6%, so 2000 is a safe threshold.
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

        // Spawn 80 mice — at cap.
        for i in 0..80i32 {
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
            count <= 80,
            "mice at cap should not exceed 80, got {count}"
        );
    }

    // -----------------------------------------------------------------------
    // prey_ai tests
    // -----------------------------------------------------------------------

    fn setup_ai() -> (World, Schedule) {
        let mut world = World::new();
        let map = TileMap::new(20, 20, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        let mut schedule = Schedule::default();
        schedule.add_systems(prey_ai);
        (world, schedule)
    }

    #[test]
    fn prey_grazes_and_moves() {
        let (mut world, mut schedule) = setup_ai();

        let start = Position::new(10, 10);
        let mut prey = PreyAnimal::new(PreySpecies::Mouse);
        prey.ai_state = PreyAiState::Grazing { dx: 1, dy: 0, ticks: 0 };
        world.spawn((prey, Health::default(), start));

        for _ in 0..60 {
            schedule.run(&mut world);
        }

        let final_pos = *world.query::<&Position>().single(&world).unwrap();
        assert!(
            final_pos != start,
            "prey should have moved from {start:?} after 60 ticks of grazing, still at {final_pos:?}"
        );
    }

    #[test]
    fn prey_flees_from_threat() {
        let (mut world, mut schedule) = setup_ai();

        // Threat at (5, 5).
        let threat = world.spawn(Position::new(5, 5)).id();

        // Prey at (7, 7), fleeing from threat.
        let start = Position::new(7, 7);
        let mut prey = PreyAnimal::new(PreySpecies::Mouse);
        prey.ai_state = PreyAiState::Fleeing { from: threat, ticks: 0 };
        world.spawn((prey, Health::default(), start));

        for _ in 0..10 {
            schedule.run(&mut world);
        }

        let final_pos = *world
            .query_filtered::<&Position, With<PreyAnimal>>()
            .single(&world)
            .unwrap();

        // Prey should have moved away — its manhattan distance from the threat
        // should be strictly greater than the starting distance of 4.
        let threat_pos = Position::new(5, 5);
        let start_dist = start.manhattan_distance(&threat_pos);
        let end_dist = final_pos.manhattan_distance(&threat_pos);
        assert!(
            end_dist > start_dist,
            "prey should flee away from threat: start_dist={start_dist}, end_dist={end_dist}, final_pos={final_pos:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Store raiding tests
    // -----------------------------------------------------------------------

    fn setup_hunger() -> (World, Schedule) {
        use crate::components::items::{Item, ItemKind, ItemLocation};

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

        // Stores building at (5, 5) with one food item.
        let stores_entity = world
            .spawn((
                Structure::new(StructureType::Stores),
                Position::new(5, 5),
                StoredItems::default(),
            ))
            .id();
        let food_entity = world
            .spawn(Item::new(
                ItemKind::RawMouse,
                0.5,
                ItemLocation::StoredIn(stores_entity),
            ))
            .id();
        world
            .entity_mut(stores_entity)
            .get_mut::<StoredItems>()
            .unwrap()
            .add(food_entity, StructureType::Stores);

        let mut schedule = Schedule::default();
        schedule.add_systems(prey_hunger);
        (world, schedule)
    }

    #[test]
    fn prey_raids_nearby_stores() {
        let (mut world, mut schedule) = setup_hunger();

        // Mouse at (5, 6) — manhattan distance 1 from stores.
        world.spawn((
            PreyAnimal::new(PreySpecies::Mouse),
            Health::default(),
            Position::new(5, 6),
        ));

        // Raiding has a 5% per-tick chance, so run enough ticks to make it
        // near-certain. P(no raid in 100 ticks) = 0.95^100 ≈ 0.006.
        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let stored = world
            .query::<&StoredItems>()
            .single(&world)
            .unwrap();
        assert!(
            stored.items.is_empty(),
            "mouse adjacent to stores should have eaten the food item within 100 ticks"
        );
    }

    #[test]
    fn fish_and_birds_do_not_raid() {
        let (mut world, mut schedule) = setup_hunger();

        // Fish at (5, 6) — adjacent to stores but fish don't raid.
        world.spawn((
            PreyAnimal::new(PreySpecies::Fish),
            Health::default(),
            Position::new(5, 6),
        ));
        // Bird at (6, 5) — also adjacent.
        world.spawn((
            PreyAnimal::new(PreySpecies::Bird),
            Health::default(),
            Position::new(6, 5),
        ));

        schedule.run(&mut world);

        let stored = world
            .query::<&StoredItems>()
            .single(&world)
            .unwrap();
        assert_eq!(
            stored.items.len(),
            1,
            "fish and birds should not raid stores"
        );
    }

    #[test]
    fn cat_near_stores_deters_raiding() {
        let (mut world, mut schedule) = setup_hunger();

        // Mouse at (5, 6) — adjacent to stores.
        world.spawn((
            PreyAnimal::new(PreySpecies::Mouse),
            Health::default(),
            Position::new(5, 6),
        ));
        // Cat at (5, 4) — within 4 tiles of stores, guarding.
        world.spawn((
            Needs::default(),
            Health::default(),
            Position::new(5, 4),
        ));

        // Run plenty of ticks — cat presence should prevent all raiding.
        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let stored = world
            .query::<&StoredItems>()
            .single(&world)
            .unwrap();
        assert_eq!(
            stored.items.len(),
            1,
            "cat guarding stores should deter mouse from raiding"
        );
    }
}
