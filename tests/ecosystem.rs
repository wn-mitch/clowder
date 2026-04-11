use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::components::physical::{Health, Position};
use clowder::components::prey::{PreyAnimal, PreyConfig, PreyDen, PreyKind, PreyState};
use clowder::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};
use clowder::resources::map::TileMap;
use clowder::resources::narrative::NarrativeLog;
use clowder::resources::rng::SimRng;
use clowder::resources::time::{SimConfig, TimeState};
use clowder::species::{self, PreyProfile, SpeciesRegistry};

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup_ecosystem() -> (World, Schedule) {
    let mut world = World::new();
    let map = TileMap::new(40, 40, clowder::resources::map::Terrain::Grass);
    world.insert_resource(map);
    world.insert_resource(SimRng::new(42));
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(TimeState::default());
    world.insert_resource(SimConfig::default());
    world.insert_resource(species::build_registry());
    world.insert_resource(clowder::components::prey::PreyDensity::default());
    world.insert_resource(clowder::resources::SimConstants::default());
    world.insert_resource(clowder::resources::SystemActivation::default());

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            clowder::systems::prey::prey_population,
            clowder::systems::prey::prey_hunger,
            clowder::systems::wildlife::predator_hunt_prey,
        )
            .chain(),
    );
    (world, schedule)
}

fn spawn_prey_of(world: &mut World, kind: PreyKind, pos: Position) {
    let registry = world.resource::<SpeciesRegistry>();
    let profile = registry.find(kind);
    let bundle = clowder::components::prey::prey_bundle(profile);
    world.spawn((bundle, pos));
}

fn spawn_rats(world: &mut World, count: usize) {
    for i in 0..count {
        let x = (i as i32) % 40;
        let y = (i as i32) / 40;
        spawn_prey_of(world, PreyKind::Rat, Position::new(x, y));
    }
}

fn spawn_foxes(world: &mut World, count: usize) {
    // Spread foxes across the map to maximise prey contact.
    for i in 0..count {
        let x = 10 + (i as i32) * 10;
        world.spawn((
            WildAnimal::new(WildSpecies::Fox),
            Health::default(),
            Position::new(x, 20),
            WildlifeAiState::Patrolling { dx: 1, dy: 0 },
        ));
    }
}

fn count_prey(world: &mut World, kind: PreyKind) -> usize {
    world
        .query::<&PreyConfig>()
        .iter(world)
        .filter(|c| c.kind == kind)
        .count()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Rats breed when left unchecked — population should grow from 5.
#[test]
fn rats_grow_when_unchecked() {
    let (mut world, mut schedule) = setup_ecosystem();

    // Add a den so breeding has a primary source.
    world.spawn((
        PreyDen::new(PreyKind::Rat, 100),
        Health::default(),
        Position::new(20, 20),
    ));
    spawn_rats(&mut world, 5);

    for _ in 0..2000 {
        schedule.run(&mut world);
    }

    let count = count_prey(&mut world, PreyKind::Rat);
    assert!(
        count > 5,
        "rats should have bred after 2000 ticks, got {count}"
    );
}

/// Population must not exceed the species cap even when spawned near it.
#[test]
fn population_respects_carrying_capacity() {
    let (mut world, mut schedule) = setup_ecosystem();
    let registry = world.resource::<SpeciesRegistry>();
    let cap = registry.find(PreyKind::Rat).population_cap();
    // Start near cap.
    spawn_rats(&mut world, cap - 1);

    for _ in 0..500 {
        schedule.run(&mut world);
    }

    let count = count_prey(&mut world, PreyKind::Rat);
    assert!(
        count <= cap,
        "rat count {count} exceeded cap of {cap}",
    );
}

/// Density pressure near the cap should produce "restless" or "overrun" log messages.
#[test]
fn density_pressure_logged_near_cap() {
    let (mut world, mut schedule) = setup_ecosystem();
    let registry = world.resource::<SpeciesRegistry>();
    let cap = registry.find(PreyKind::Rat).population_cap();
    // Start deep in the crowded zone (density_pressure < 0.1).
    // Use a large log capacity so early messages aren't evicted.
    world.resource_mut::<NarrativeLog>().capacity = 5000;
    spawn_rats(&mut world, cap);

    for _ in 0..5000 {
        schedule.run(&mut world);
    }

    let log = world.resource::<NarrativeLog>();
    let has_density_message = log.entries.iter().any(|e| {
        e.text.contains("restless") || e.text.contains("overrun")
    });

    assert!(
        has_density_message,
        "expected a density-pressure narrative message after 5000 ticks near cap"
    );
}

/// Foxes should kill at least some mice over 1000 ticks.
#[test]
fn predators_thin_prey_populations() {
    let (mut world, mut schedule) = setup_ecosystem();
    // 20 mice spread tightly so foxes can reach them.
    for i in 0..20i32 {
        spawn_prey_of(
            &mut world,
            PreyKind::Mouse,
            // Cluster near fox starting positions.
            Position::new(10 + i % 10, 20 + i / 10),
        );
    }
    spawn_foxes(&mut world, 2);

    for _ in 0..1000 {
        schedule.run(&mut world);
    }

    let count = count_prey(&mut world, PreyKind::Mouse);
    assert!(
        count < 20,
        "foxes should have eaten some mice after 1000 ticks, got {count}"
    );
}
