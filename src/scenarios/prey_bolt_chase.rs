//! Ticket 266 — prey Bolt election under a committed close threat.
//!
//! A max-alert rabbit holds `Alert` two tiles from an adult cat. The
//! affordance writer computes the real prey-perceiver rows (Chase from
//! the cat's side, Bolt from the rabbit's); on the election cadence the
//! `prey_bolt` score clears the threshold and the rabbit enters
//! `PreyAiState::Bolting` — the transition is *named*
//! (`Feature::PreyBoltElected`), not a silent state flip.
//!
//! De-raced per the scenario-geometry discipline: the rabbit starts
//! pre-Alert at alertness 1.0 (the probabilistic detection chain is
//! covered by `prey_alert_detects_nearby_cat` at the unit layer) and
//! the election cadence is pinned to 1 so the rabbit's 10-tick freeze
//! window cannot slip past a sparse cadence. Distance 2 keeps the
//! cat's Chase read committed (proximity ≈ 0.87) while leaving the
//! rabbit outside the same-tile grab window at t0.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::{PreyAiState, PreyKind, PreyState};

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::CatPreset;
use super::Scenario;

const CAT_POS: Position = Position::new(18, 20);
const RABBIT_POS: Position = Position::new(20, 20);

pub static SCENARIO: Scenario = Scenario {
    name: "prey_bolt_chase",
    default_focal: "Whisker",
    default_ticks: 30,
    setup,
    expected_features: &["PreyBoltElected"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    {
        let mut constants = world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        // Election every tick — the rabbit's freeze window (10 ticks)
        // must not race a sparse cadence in a 30-tick run.
        constants.prey.prey_ai_cadence_ticks = 1;
    }

    let cat = spawn_cat(world, CatPreset::adult("Whisker", CAT_POS));

    let rabbit = spawn_prey_at(world, RABBIT_POS, PreyKind::Rabbit);
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::resources::system_activation::{Feature, SystemActivation};
    use crate::scenarios::runner::build_scenario_app;

    #[test]
    fn committed_threat_elects_a_named_bolt() {
        let mut app = build_scenario_app(42, &SCENARIO, SCENARIO.default_focal);

        let mut bolted = false;
        for _ in 0..SCENARIO.default_ticks {
            app.update();
            let world = app.world_mut();
            let mut q = world.query::<&PreyState>();
            if q.iter(world)
                .any(|s| matches!(s.ai_state, PreyAiState::Bolting { .. }))
            {
                bolted = true;
                break;
            }
        }
        assert!(
            bolted,
            "the alert rabbit two tiles from a cat should elect Bolt within the run"
        );

        let world = app.world_mut();
        let elected = world
            .resource::<SystemActivation>()
            .counts
            .get(&Feature::PreyBoltElected)
            .copied()
            .unwrap_or(0);
        assert!(elected >= 1, "the Bolt election must be named");
    }
}
