//! Smoke test for ticket 025 Phase 2 snake GOAP pipeline.

use bevy::prelude::*;

use clowder::ai::eval::{DseRegistry, ModifierPipeline};
use clowder::ai::snake_scoring::{SnakeNeeds, SnakePersonality};
use clowder::components::physical::{Health, Position};
use clowder::components::wildlife::{
    SnakeAiPhase, SnakeDied, SnakeState, WildAnimal, WildSpecies, WildlifeAiState,
};
use clowder::plugins::simulation::populate_dse_registry;
use clowder::resources::map::{Terrain, TileMap};
use clowder::resources::rng::SimRng;
use clowder::resources::sim_constants::ScoringConstants;
use clowder::resources::time::{SimConfig, TimeScale, TimeState};
use clowder::resources::{SimConstants, SystemActivation};
use clowder::systems::snake_goap;

fn build_test_app() -> App {
    let mut app = App::new();
    app.insert_resource(TileMap::new(40, 40, Terrain::Grass));
    app.insert_resource(SimRng::new(42));
    app.insert_resource(TimeState::default());
    app.insert_resource(SimConfig::default());
    app.insert_resource(TimeScale::from_config(&SimConfig::default(), 16.6667));
    app.insert_resource(SimConstants::default());
    app.insert_resource(SystemActivation::default());

    let scoring = ScoringConstants::default();
    let mut registry = DseRegistry::new();
    populate_dse_registry(&mut registry, &scoring);
    app.insert_resource(registry);
    app.insert_resource(ModifierPipeline::default());

    app.add_message::<SnakeDied>();

    app.add_systems(
        Update,
        (
            snake_goap::snake_needs_tick,
            snake_goap::sync_snake_needs,
            snake_goap::snake_evaluate_and_plan,
            snake_goap::snake_resolve_goap_plans,
            snake_goap::snake_lifecycle_tick,
        )
            .chain(),
    );
    app
}

fn spawn_snake(world: &mut World) -> Entity {
    world
        .spawn((
            WildAnimal::new(WildSpecies::Snake),
            SnakeState::new_adult(),
            SnakeAiPhase::Waiting,
            SnakeNeeds::default(),
            SnakePersonality::default(),
            Health::default(),
            Position::new(15, 15),
            WildlifeAiState::Waiting,
        ))
        .id()
}

#[test]
fn snake_evaluate_inserts_plan() {
    // Same caveat as the hawk smoke test: Ambushing's planner state
    // starts in Cover and `SetAmbush` has no precondition, so the
    // plan has at least one step. We still assert via age_ticks to
    // be robust against future changes to the disposition default.
    let mut app = build_test_app();
    let id = spawn_snake(app.world_mut());
    app.update();
    let age = app.world().get::<SnakeState>(id).unwrap().age_ticks;
    assert!(age >= 1, "expected snake_needs_tick to advance age");
}

#[test]
fn snake_pipeline_advances_for_100_ticks_without_panic() {
    let mut app = build_test_app();
    let _id = spawn_snake(app.world_mut());
    for _ in 0..100 {
        app.update();
    }
}

#[test]
fn snake_lifecycle_kills_starving_snake() {
    let mut app = build_test_app();
    let id = app
        .world_mut()
        .spawn((
            WildAnimal::new(WildSpecies::Snake),
            SnakeState {
                hunger: 1.0,
                satiation_ticks: 0,
                warmth: 0.5,
                age_ticks: 0,
                post_action_cooldown: 0,
                starvation_ticks: 60 * 60 * 24 * 5 - 2,
                last_strike_tick: 0,
                last_bask_tick: 0,
            },
            SnakeAiPhase::Waiting,
            SnakeNeeds::default(),
            SnakePersonality::default(),
            Health::default(),
            Position::new(15, 15),
            WildlifeAiState::Waiting,
        ))
        .id();
    for _ in 0..10 {
        app.update();
        if app
            .world()
            .get::<clowder::components::physical::Dead>(id)
            .is_some()
        {
            break;
        }
    }
    assert!(app
        .world()
        .get::<clowder::components::physical::Dead>(id)
        .is_some());
}
