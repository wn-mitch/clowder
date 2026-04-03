use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::ai::{Action, CurrentAction};
use clowder::components::mental::{Memory, Mood};
use clowder::components::physical::{Needs, Position};
use clowder::components::skills::Skills;
use clowder::resources::food::FoodStores;
use clowder::resources::map::{Terrain, TileMap};
use clowder::resources::relationships::Relationships;
use clowder::resources::rng::SimRng;
use clowder::resources::time::TimeState;
use clowder::systems::actions::resolve_actions;

fn setup_world() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
    world.insert_resource(FoodStores::default());
    world.insert_resource(SimRng::new(42));
    world.insert_resource(TimeState::default());
    world.insert_resource(clowder::resources::time::SimConfig::default());
    world.insert_resource(Relationships::default());
    let mut schedule = Schedule::default();
    schedule.add_systems(resolve_actions);
    (world, schedule)
}

/// Mentor action restores the mentor's mastery need when adjacent to apprentice.
#[test]
fn mentoring_restores_mastery() {
    let (mut world, mut schedule) = setup_world();

    let mut mentor_needs = Needs::default();
    mentor_needs.mastery = 0.3;

    let mut mentor_skills = Skills::default();
    mentor_skills.hunting = 0.8; // high enough to teach

    // Spawn apprentice adjacent to mentor.
    let apprentice = world.spawn((
        CurrentAction::default(),
        Needs::default(),
        Position::new(5, 6),
        Skills::default(), // hunting defaults to 0.1
        Memory::default(),
        Mood::default(),
    )).id();

    let mentor = world.spawn((
        CurrentAction {
            action: Action::Mentor,
            ticks_remaining: 10,
            target_position: Some(Position::new(5, 6)),
            target_entity: Some(apprentice),
            last_scores: Vec::new(),
        },
        mentor_needs,
        Position::new(5, 5), // adjacent
        mentor_skills,
        Memory::default(),
        Mood::default(),
    )).id();

    let mastery_before = world.get::<Needs>(mentor).unwrap().mastery;

    schedule.run(&mut world);

    let mastery_after = world.get::<Needs>(mentor).unwrap().mastery;
    assert!(
        mastery_after > mastery_before,
        "mentor mastery should increase; before={mastery_before}, after={mastery_after}"
    );
}

/// Mentor action grows the apprentice's skill at 2x rate when adjacent.
#[test]
fn mentoring_grows_apprentice_skill() {
    let (mut world, mut schedule) = setup_world();

    let mut mentor_skills = Skills::default();
    mentor_skills.hunting = 0.8;

    let apprentice_skills = Skills::default(); // hunting = 0.1

    let apprentice = world.spawn((
        CurrentAction::default(),
        Needs::default(),
        Position::new(5, 6),
        apprentice_skills,
        Memory::default(),
        Mood::default(),
    )).id();

    world.spawn((
        CurrentAction {
            action: Action::Mentor,
            ticks_remaining: 10,
            target_position: Some(Position::new(5, 6)),
            target_entity: Some(apprentice),
            last_scores: Vec::new(),
        },
        Needs::default(),
        Position::new(5, 5),
        mentor_skills,
        Memory::default(),
        Mood::default(),
    ));

    let hunting_before = world.get::<Skills>(apprentice).unwrap().hunting;

    schedule.run(&mut world);

    let hunting_after = world.get::<Skills>(apprentice).unwrap().hunting;
    assert!(
        hunting_after > hunting_before,
        "apprentice hunting should grow; before={hunting_before}, after={hunting_after}"
    );
}

/// Mentor builds fondness with the apprentice over time.
#[test]
fn mentoring_builds_fondness() {
    let (mut world, mut schedule) = setup_world();

    let mut mentor_skills = Skills::default();
    mentor_skills.hunting = 0.8;

    let apprentice = world.spawn((
        CurrentAction::default(),
        Needs::default(),
        Position::new(5, 6),
        Skills::default(),
        Memory::default(),
        Mood::default(),
    )).id();

    let mentor = world.spawn((
        CurrentAction {
            action: Action::Mentor,
            ticks_remaining: 10,
            target_position: Some(Position::new(5, 6)),
            target_entity: Some(apprentice),
            last_scores: Vec::new(),
        },
        Needs::default(),
        Position::new(5, 5),
        mentor_skills,
        Memory::default(),
        Mood::default(),
    )).id();

    // Init relationship.
    {
        let mut rels = world.resource_mut::<Relationships>();
        let rel = rels.get_or_insert(mentor, apprentice);
        rel.fondness = 0.0;
        rel.familiarity = 0.0;
    }

    schedule.run(&mut world);

    let rels = world.resource::<Relationships>();
    let rel = rels.get(mentor, apprentice).unwrap();
    assert!(
        rel.fondness > 0.0,
        "fondness should increase after mentoring; got {}",
        rel.fondness
    );
}
