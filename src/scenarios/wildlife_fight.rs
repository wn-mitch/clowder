//! Wildlife-fight scenario — a bold healthy adult cat encounters a hawk
//! at adjacent range. Tests the Fight / Flee / Hide DSE branching under
//! threat presence.
//!
//! Expected on a healthy build: the bold cat enters Fight or Flee
//! (depending on combat-winnability heuristics), not Wander or Idle.

use bevy_ecs::world::World;

use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat, spawn_hawk_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "wildlife_fight",
    default_focal: "Briar",
    default_ticks: 15,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let cat_pos = Position::new(20, 20);
    let hawk_pos = Position::new(21, 20);

    // Briar: bold, brave, healthy. Should pick Fight when threatened.
    let _briar = spawn_cat(
        world,
        CatPreset::adult("Briar", cat_pos)
            .with_personality(|p| {
                p.boldness = 0.95;
                p.anxiety = 0.1;
                p.temper = 0.7;
                p.pride = 0.7;
            })
            .with_marker(MarkerKind::Adult),
    );

    let _hawk = spawn_hawk_at(world, hawk_pos);
}
