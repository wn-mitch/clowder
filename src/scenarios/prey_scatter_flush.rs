//! Ticket 266 — herd flush: ScatterGroup out-ranks Bolt for grouped prey.
//!
//! Five max-alert rabbits cluster two-to-three tiles from an adult cat.
//! The affordance writer computes the real prey-perceiver rows — the
//! group census saturates the ScatterGroup heuristic's herd input — and
//! on the election cadence the flush out-ranks the individual bolt:
//! members enter `PreyAiState::Scattering` and the transitions are
//! *named* (`Feature::PreyScatterElected`).
//!
//! De-raced like `prey_bolt_chase`: rabbits start pre-Alert at
//! alertness 1.0, cadence pinned to 1. The divergent-heading geometry
//! (parity-mirrored rotation) is pinned by the `scattering_herd_diverges`
//! unit test; this scenario owns the integrated election chain.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::{PreyAiState, PreyKind, PreyState};

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::CatPreset;
use super::Scenario;

const CAT_POS: Position = Position::new(17, 20);

pub static SCENARIO: Scenario = Scenario {
    name: "prey_scatter_flush",
    default_focal: "Whisker",
    default_ticks: 30,
    setup,
    expected_features: &["PreyScatterElected"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    {
        let mut constants = world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        constants.prey.prey_ai_cadence_ticks = 1;
    }

    let cat = spawn_cat(world, CatPreset::adult("Whisker", CAT_POS));

    let herd = [
        Position::new(20, 20),
        Position::new(21, 20),
        Position::new(20, 21),
        Position::new(21, 21),
        Position::new(20, 19),
    ];
    for pos in herd {
        let rabbit = spawn_prey_at(world, pos, PreyKind::Rabbit);
        let mut rabbit_entity = world.entity_mut(rabbit);
        let mut state = rabbit_entity
            .get_mut::<PreyState>()
            .expect("prey bundle carries PreyState");
        state.alertness = 1.0;
        state.ai_state = PreyAiState::Alert {
            threat: cat,
            ticks: 0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::resources::system_activation::{Feature, SystemActivation};
    use crate::scenarios::runner::build_scenario_app;

    #[test]
    fn grouped_rabbits_elect_the_flush() {
        let mut app = build_scenario_app(42, &SCENARIO, SCENARIO.default_focal);

        let mut max_scattering = 0usize;
        for _ in 0..SCENARIO.default_ticks {
            app.update();
            let world = app.world_mut();
            let mut q = world.query::<&PreyState>();
            let scattering = q
                .iter(world)
                .filter(|s| matches!(s.ai_state, PreyAiState::Scattering { .. }))
                .count();
            max_scattering = max_scattering.max(scattering);
            if max_scattering >= 2 {
                break;
            }
        }
        assert!(
            max_scattering >= 2,
            "the herd should flush (≥2 Scattering at once); peak was {max_scattering}"
        );

        let world = app.world_mut();
        let elected = world
            .resource::<SystemActivation>()
            .counts
            .get(&Feature::PreyScatterElected)
            .copied()
            .unwrap_or(0);
        assert!(elected >= 2, "the flush elections must be named");
    }
}
