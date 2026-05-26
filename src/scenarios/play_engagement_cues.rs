//! Ticket 279 — play-engagement perception cue microexperiment. Sister to
//! `belief_affordance_dse_consumers.rs` (258/263 consumer pattern).
//!
//! Verifies the emit → integrate path for the play-engagement cues that 279
//! wires into `belief_integrator`: `PlayBow`, `ReciprocalAdvance`, and
//! `SustainedCoPresence`. The scenario asserts on the **deterministic**
//! `SustainedCoPresence` path — two adjacent, low-energy (sleeping) cats stay
//! within `passive_familiarity_range` from the cache-bootstrap tick, so the
//! per-pair co-presence counter accumulates monotonically and crosses the
//! emit threshold without any RNG. (PlayBow's emit is probabilistic, so it's
//! exercised but not asserted — the deterministic SustainedCoPresence lift is
//! the gate signal.)
//!
//! After ~40 ticks the tracker has emitted at least once and the integrator
//! has lifted `perceived_intent_clarity` on each cat's `MentalModel` of the
//! other. The microexperiment runs in ~1-2s — the cheap triage that catches
//! integrator-wiring regressions before a 15-min soak.

use bevy_ecs::world::World;

use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const FOCAL_NAME: &str = "Bramble";
const PARTNER_NAME: &str = "Clover";

pub static SCENARIO_PLAY_ENGAGEMENT_CUES: Scenario = Scenario {
    name: "play_engagement_cues",
    default_focal: FOCAL_NAME,
    // Threshold is 30 consecutive co-present ticks; +1 for the Chain-4 →
    // Chain-2b integrator latency, plus headroom for the startup tick.
    default_ticks: 40,
    setup: setup_play_engagement_cues,
    expected_features: &[],
};

fn setup_play_engagement_cues(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    // Two adjacent, highly-playful cats with depleted energy so Sleep wins
    // and they stay co-located (stationary) for the whole run — the
    // SustainedCoPresence counter needs an uninterrupted co-presence window.
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_personality(|p| p.playfulness = 0.9)
            .with_needs(|n| n.energy = 0.05)
            .with_marker(MarkerKind::Adult),
    );
    spawn_cat(
        world,
        CatPreset::adult(PARTNER_NAME, Position::new(21, 20))
            .with_personality(|p| p.playfulness = 0.9)
            .with_needs(|n| n.energy = 0.05)
            .with_marker(MarkerKind::Adult),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::beliefs::CatBeliefs;
    use crate::components::identity::{Name, Species};
    use crate::scenarios::runner::build_scenario_app;
    use bevy_ecs::prelude::*;

    fn cat_by_name(world: &mut World, name: &str) -> Entity {
        let mut q = world.query_filtered::<(Entity, &Name), With<Species>>();
        q.iter(world)
            .find(|(_, n)| n.0 == name)
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("cat named {name:?} not found"))
    }

    fn run_scenario_ticks(scenario: &Scenario, ticks: u32) -> bevy::app::App {
        let mut app = build_scenario_app(42, scenario, scenario.default_focal);
        app.update();
        for _ in 0..ticks {
            app.update();
        }
        app
    }

    fn intent_clarity(world: &World, witness: Entity, actor: Entity) -> f32 {
        world
            .get::<CatBeliefs>(witness)
            .expect("witness has CatBeliefs")
            .models
            .get(&actor)
            .map(|m| m.perceived_intent_clarity.value)
            .unwrap_or(0.0)
    }

    #[test]
    fn sustained_copresence_lifts_mutual_intent_clarity() {
        let mut app = run_scenario_ticks(&SCENARIO_PLAY_ENGAGEMENT_CUES, 40);
        let world = app.world_mut();
        let bramble = cat_by_name(world, FOCAL_NAME);
        let clover = cat_by_name(world, PARTNER_NAME);

        let bramble_on_clover = intent_clarity(world, bramble, clover);
        let clover_on_bramble = intent_clarity(world, clover, bramble);

        assert!(
            bramble_on_clover > 0.0,
            "co-present cats should accrue perceived_intent_clarity; \
             Bramble's model of Clover was {bramble_on_clover}"
        );
        assert!(
            clover_on_bramble > 0.0,
            "co-presence emits symmetrically; Clover's model of Bramble \
             was {clover_on_bramble}"
        );
    }
}
