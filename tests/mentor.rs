use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::ai::CurrentAction;
use clowder::components::hunting_priors::HuntingPriors;
use clowder::components::identity::{Gender, Name};
use clowder::components::magic::Inventory;
use clowder::components::mental::Mood;
use clowder::components::personality::Personality;
use clowder::components::physical::{Needs, Position};
use clowder::components::skills::Skills;
use clowder::components::task_chain::{FailurePolicy, StepKind, StepStatus, TaskChain, TaskStep};
use clowder::resources::colony_hunting_map::ColonyHuntingMap;
use clowder::resources::map::{Terrain, TileMap};
use clowder::resources::narrative::NarrativeLog;
use clowder::resources::relationships::Relationships;
use clowder::resources::rng::SimRng;
use clowder::resources::time::{SimConfig, TimeState};
use clowder::resources::weather::WeatherState;
use clowder::resources::wind::WindState;
use clowder::systems::disposition::resolve_disposition_chains;

fn setup_world() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
    world.insert_resource(SimRng::new(42));
    world.insert_resource(TimeState::default());
    world.insert_resource(Relationships::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(WindState::default());
    world.insert_resource(ColonyHuntingMap::new(20, 20));
    world.insert_resource(clowder::resources::ExplorationMap::new(20, 20));
    world.insert_resource(clowder::resources::SimConstants::default());
    world.insert_resource(SimConfig::default());
    world.insert_resource(WeatherState::default());
    world.insert_resource(clowder::species::build_registry());
    world.insert_resource(clowder::components::prey::PreyDensity::default());
    world.insert_resource(clowder::resources::PreyScentMap::default());
    bevy_ecs::message::MessageRegistry::register_message::<clowder::components::prey::PreyKilled>(
        &mut world,
    );
    bevy_ecs::message::MessageRegistry::register_message::<clowder::components::prey::DenRaided>(
        &mut world,
    );
    bevy_ecs::message::MessageRegistry::register_message::<
        clowder::components::goap_plan::PlanNarrative,
    >(&mut world);
    let mut schedule = Schedule::default();
    schedule.add_systems(resolve_disposition_chains);
    (world, schedule)
}

fn mentor_chain(target: Entity) -> TaskChain {
    let mut step = TaskStep::new(StepKind::MentorCat).with_entity(target);
    step.status = StepStatus::InProgress { ticks_elapsed: 0 };
    TaskChain::new(vec![step], FailurePolicy::AbortChain)
}

/// Mentor action restores the mentor's mastery need.
#[test]
fn mentoring_restores_mastery() {
    let (mut world, mut schedule) = setup_world();

    let mut mentor_needs = Needs::default();
    mentor_needs.mastery = 0.3;

    let mut mentor_skills = Skills::default();
    mentor_skills.hunting = 0.8;

    let apprentice = world
        .spawn((
            CurrentAction::default(),
            Needs::default(),
            Position::new(5, 6),
            Skills::default(),
            Inventory::default(),
            Personality::random(&mut rand::rng()),
            Name("Apprentice".to_string()),
            Mood::default(),
        ))
        .id();

    let mentor = world
        .spawn((
            mentor_chain(apprentice),
            CurrentAction::default(),
            mentor_needs,
            Position::new(5, 5),
            mentor_skills,
            Inventory::default(),
            Personality::random(&mut rand::rng()),
            Name("Mentor".to_string()),
            Gender::Tom,
            HuntingPriors::default(),
            Mood::default(),
        ))
        .id();

    let mastery_before = world.get::<Needs>(mentor).unwrap().mastery;

    // Run enough ticks for per-tick mastery gain.
    schedule.run(&mut world);

    let mastery_after = world.get::<Needs>(mentor).unwrap().mastery;
    assert!(
        mastery_after > mastery_before,
        "mentor mastery should increase; before={mastery_before}, after={mastery_after}"
    );
}

/// Mentor action grows the apprentice's skill at 2x rate.
#[test]
fn mentoring_grows_apprentice_skill() {
    let (mut world, mut schedule) = setup_world();

    let mut mentor_skills = Skills::default();
    mentor_skills.hunting = 0.8;

    let apprentice = world
        .spawn((
            CurrentAction::default(),
            Needs::default(),
            Position::new(5, 6),
            Skills::default(), // hunting = 0.1
            Inventory::default(),
            Personality::random(&mut rand::rng()),
            Name("Apprentice".to_string()),
            Mood::default(),
        ))
        .id();

    // Chain with ticks_elapsed already at 12 so completion fires on next run.
    let mut step = TaskStep::new(StepKind::MentorCat).with_entity(apprentice);
    step.status = StepStatus::InProgress { ticks_elapsed: 11 };
    let chain = TaskChain::new(vec![step], FailurePolicy::AbortChain);

    world.spawn((
        chain,
        CurrentAction::default(),
        Needs::default(),
        Position::new(5, 5),
        mentor_skills,
        Inventory::default(),
        Personality::random(&mut rand::rng()),
        Name("Mentor".to_string()),
        Gender::Tom,
        HuntingPriors::default(),
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

    let apprentice = world
        .spawn((
            CurrentAction::default(),
            Needs::default(),
            Position::new(5, 6),
            Skills::default(),
            Inventory::default(),
            Personality::random(&mut rand::rng()),
            Name("Apprentice".to_string()),
            Mood::default(),
        ))
        .id();

    let mentor = world
        .spawn((
            mentor_chain(apprentice),
            CurrentAction::default(),
            Needs::default(),
            Position::new(5, 5),
            mentor_skills,
            Inventory::default(),
            Personality::random(&mut rand::rng()),
            Name("Mentor".to_string()),
            Gender::Tom,
            HuntingPriors::default(),
            Mood::default(),
        ))
        .id();

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
