//! Exploration ranging scenario — a curious, well-rested adult cat with
//! all physiological needs satisfied is placed in a fresh world. Tests
//! whether Explore wins over Wander when high-curiosity / low-purpose
//! state should drive purposeful movement, and whether the cat actually
//! ranges out (position changes substantively) over the run.

use bevy_ecs::world::World;

use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "exploration_ranging",
    default_focal: "Cinder",
    default_ticks: 60,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Cinder: high curiosity + boldness, well-rested, well-fed, low
    // purpose. Classic explorer profile.
    let _cinder = spawn_cat(
        world,
        CatPreset::adult("Cinder", Position::new(20, 20))
            .with_personality(|p| {
                p.curiosity = 0.95;
                p.boldness = 0.8;
                p.independence = 0.7;
                p.anxiety = 0.2;
            })
            .with_needs(|n| {
                n.hunger = 0.95;
                n.energy = 0.95;
                n.safety = 1.0;
                n.purpose = 0.05; // strong drive toward Explore / mastery
                n.mastery = 0.2;
            })
            .with_marker(MarkerKind::Adult),
    );
}
