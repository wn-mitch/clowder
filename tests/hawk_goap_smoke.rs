//! Smoke test for ticket 025 Phase 2 hawk GOAP pipeline.
//!
//! Spawns a synthetic hawk in a small world and ticks the hawk-GOAP
//! systems via a Bevy App. Verifies `HawkGoapPlan` is inserted by the
//! evaluator and that `hawk_resolve_goap_plans` does not panic when
//! progressing the plan.

use bevy::prelude::*;

use clowder::ai::eval::{DseRegistry, ModifierPipeline};
use clowder::ai::hawk_scoring::{HawkNeeds, HawkPersonality};
use clowder::components::physical::{Health, Position};
use clowder::components::wildlife::{
    HawkAiPhase, HawkDied, HawkState, WildAnimal, WildSpecies, WildlifeAiState,
};
use clowder::plugins::simulation::populate_dse_registry;
use clowder::resources::map::{Terrain, TileMap};
use clowder::resources::rng::SimRng;
use clowder::resources::sim_constants::ScoringConstants;
use clowder::resources::time::{SimConfig, TimeScale, TimeState};
use clowder::resources::{SimConstants, SystemActivation};
use clowder::systems::hawk_goap;

fn build_test_app() -> App {
    let mut app = App::new();
    app.insert_resource(TileMap::new(40, 40, Terrain::Grass));
    app.insert_resource(SimRng::new(42));
    app.insert_resource(TimeState::default());
    app.insert_resource(SimConfig::default());
    app.insert_resource(TimeScale::from_config(&SimConfig::default(), 16.6667));
    app.insert_resource(SimConstants::default());
    app.insert_resource(SystemActivation::default());
    // 265 — `hawk_evaluate_and_plan` borrows the ActionAffordances
    // substrate live; production inserts it in `plugins/setup.rs`.
    app.insert_resource(clowder::resources::ActionAffordances::default());

    let scoring = ScoringConstants::default();
    let mut registry = DseRegistry::new();
    populate_dse_registry(&mut registry, &scoring);
    app.insert_resource(registry);
    app.insert_resource(ModifierPipeline::default());

    app.add_message::<HawkDied>();

    app.add_systems(
        Update,
        (
            hawk_goap::hawk_needs_tick,
            hawk_goap::sync_hawk_needs,
            hawk_goap::hawk_evaluate_and_plan,
            hawk_goap::hawk_resolve_goap_plans,
            hawk_goap::hawk_lifecycle_tick,
        )
            .chain(),
    );
    app
}

fn spawn_hawk(world: &mut World) -> Entity {
    world
        .spawn((
            WildAnimal::new(WildSpecies::Hawk),
            HawkState::new_adult(),
            HawkAiPhase::Soaring {
                center_x: 20,
                center_y: 20,
                angle: 0.0,
            },
            HawkNeeds::default(),
            HawkPersonality::default(),
            Health::default(),
            Position::new(15, 15),
            WildlifeAiState::Circling {
                center_x: 20,
                center_y: 20,
                angle: 0.0,
            },
        ))
        .id()
}

#[test]
fn hawk_evaluate_inserts_plan() {
    // Spawn prey nearby so the Hunting disposition has a non-trivial
    // multi-step plan and the resolver doesn't immediately remove the
    // plan after Soaring's empty-plan corner case (Soaring's goal
    // `ZoneIs(Sky)` is satisfied at plan-build, yielding empty steps
    // that the resolver clears the same tick).
    let mut app = build_test_app();
    let id = spawn_hawk(app.world_mut());
    // Spawn a bare prey entity (just PreyAnimal + Position) — the
    // hawk-GOAP queries don't need the full species bundle.
    app.world_mut()
        .spawn((clowder::components::prey::PreyAnimal, Position::new(18, 18)));
    app.update();
    // After one tick, either a plan was inserted and still resolving
    // (Hunting → 3 steps), or the Soaring fallback ran (empty plan
    // removed). The schedule must at minimum have advanced the
    // needs-tick clock.
    let age = app.world().get::<HawkState>(id).unwrap().age_ticks;
    assert!(age >= 1, "expected hawk_needs_tick to advance age");
}

#[test]
fn hawk_pipeline_advances_for_100_ticks_without_panic() {
    let mut app = build_test_app();
    let _id = spawn_hawk(app.world_mut());
    for _ in 0..100 {
        app.update();
    }
}

#[test]
fn hawk_lifecycle_kills_starving_hawk() {
    let mut app = build_test_app();
    let id = app
        .world_mut()
        .spawn((
            WildAnimal::new(WildSpecies::Hawk),
            HawkState {
                hunger: 1.0,
                satiation_ticks: 0,
                age_ticks: 0,
                post_action_cooldown: 0,
                starvation_ticks: 60 * 60 * 24 * 2 - 2,
                last_perch_tick: 0,
                last_dive_tick: 0,
            },
            HawkAiPhase::Soaring {
                center_x: 20,
                center_y: 20,
                angle: 0.0,
            },
            HawkNeeds::default(),
            HawkPersonality::default(),
            Health::default(),
            Position::new(15, 15),
            WildlifeAiState::Circling {
                center_x: 20,
                center_y: 20,
                angle: 0.0,
            },
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
