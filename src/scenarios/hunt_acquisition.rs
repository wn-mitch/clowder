//! Hunt acquisition-through-kill scenario — a hungry skilled hunter cat
//! is placed near a single mouse on flat terrain. Tests Hunt DSE
//! eligibility (`CanHunt` marker), target_taking selection, and the full
//! locate→stalk→pounce→kill chain.
//!
//! Expected on a healthy build: focal cat picks Hunting, locks onto the
//! mouse, kills it within ~30 ticks. If the kill never happens the
//! scenario surfaces *which* step in the chain stalls.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "hunt_acquisition_to_kill",
    default_focal: "Talon",
    default_ticks: 30,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Hungry, skilled, bold — should commit to Hunting.
    let _talon = spawn_cat(
        world,
        CatPreset::adult("Talon", Position::new(20, 20))
            .with_personality(|p| {
                p.boldness = 0.85;
                p.diligence = 0.7;
                p.patience = 0.7;
            })
            .with_needs(|n| {
                // Hungry but not starving — Eating disposition would
                // otherwise dominate via the Maslow physiological gate.
                n.hunger = 0.45;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Mouse 4 tiles away — well within sense range, short pounce path.
    let _mouse = spawn_prey_at(world, Position::new(24, 20), PreyKind::Mouse);
}
