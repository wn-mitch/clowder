use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::ai::CurrentAction;
use clowder::components::identity::{Age, Name, Species};
use clowder::components::mental::{Memory, Mood};
use clowder::components::physical::{Health, Needs, Position};
use clowder::components::skills::{Corruption, MagicAffinity, Training};
use clowder::resources::{
    NarrativeLog, SimConfig, SimRng, TimeState, TileMap, Terrain, WeatherState,
};
use clowder::world_gen::colony::{find_colony_site, generate_starting_cats};
use clowder::world_gen::terrain::generate_terrain;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_world(seed: u64) -> World {
    let config = SimConfig {
        seed,
        ..SimConfig::default()
    };
    let mut sim_rng = SimRng::new(seed);

    let mut map = generate_terrain(80, 60, &mut sim_rng.rng);

    let colony_site = find_colony_site(&map, &mut sim_rng.rng);

    map.set(colony_site.x, colony_site.y, Terrain::Hearth);
    let den_x = (colony_site.x - 2).max(0);
    map.set(den_x, colony_site.y, Terrain::Den);
    let stores_x = (colony_site.x + 2).min(map.width - 1);
    map.set(stores_x, colony_site.y, Terrain::Stores);

    let cat_blueprints = generate_starting_cats(8, &mut sim_rng.rng);

    let mut world = World::new();
    world.insert_resource(TimeState::default());
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(map);
    world.insert_resource(sim_rng);

    for (i, cat) in cat_blueprints.into_iter().enumerate() {
        let offset_x = (i as i32 % 5) - 2;
        let offset_y = (i as i32 / 5) - 1;

        let (spawn_x, spawn_y) = {
            let map_ref = world.resource::<TileMap>();
            (
                (colony_site.x + offset_x).clamp(0, map_ref.width - 1),
                (colony_site.y + offset_y).clamp(0, map_ref.height - 1),
            )
        };

        world.spawn((
            (
                Name(cat.name),
                Species::Cat,
                Age { born_tick: 0 },
                cat.gender,
                cat.orientation,
                cat.personality,
                cat.appearance,
                Position::new(spawn_x, spawn_y),
                Health::default(),
                Needs::default(),
                Mood::default(),
                Memory::default(),
            ),
            (
                cat.skills,
                MagicAffinity(cat.magic_affinity),
                Corruption(0.0),
                Training::default(),
                CurrentAction::default(),
            ),
        ));
    }

    world
}

fn build_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            clowder::systems::time::advance_time,
            clowder::systems::weather::update_weather,
            clowder::systems::needs::decay_needs,
            clowder::systems::ai::evaluate_actions,
            clowder::systems::actions::resolve_actions,
            clowder::systems::narrative::generate_narrative,
        )
            .chain(),
    );
    schedule
}

fn run_simulation(seed: u64, ticks: u64) -> Vec<String> {
    let mut world = setup_world(seed);
    let mut schedule = build_schedule();

    for _ in 0..ticks {
        schedule.run(&mut world);
    }

    world
        .resource::<NarrativeLog>()
        .entries
        .iter()
        .map(|e| e.text.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn simulation_is_deterministic() {
    let first = run_simulation(42, 100);
    let second = run_simulation(42, 100);

    assert_eq!(
        first.len(),
        second.len(),
        "runs produced different numbers of narrative entries: {} vs {}",
        first.len(),
        second.len()
    );

    for (i, (a, b)) in first.iter().zip(second.iter()).enumerate() {
        assert_eq!(
            a, b,
            "narrative entry {i} differs between runs:\n  run1: {a}\n  run2: {b}"
        );
    }
}

#[test]
fn cats_eat_when_hungry() {
    let mut world = setup_world(42);
    let mut schedule = build_schedule();

    // Drive all cats to near-starving hunger before the schedule runs.
    let entity_ids: Vec<Entity> = world
        .query::<Entity>()
        .iter(&world)
        .collect();

    for entity in entity_ids {
        if let Some(mut needs) = world.get_mut::<Needs>(entity) {
            needs.hunger = 0.1;
        }
    }

    for _ in 0..50 {
        schedule.run(&mut world);
    }

    let any_hunger_improved = world
        .query::<&Needs>()
        .iter(&world)
        .any(|needs| needs.hunger > 0.15);

    assert!(
        any_hunger_improved,
        "no cat's hunger improved above 0.15 after 50 ticks — eating may be broken"
    );
}

#[test]
fn simulation_runs_1000_ticks_without_panic() {
    let mut world = setup_world(42);
    let mut schedule = build_schedule();

    for _ in 0..1000 {
        schedule.run(&mut world);
    }
    // If we reach here, no panic occurred.
}
