//! Fondness / kitten-imprint scenario — a kitten and its mother spawn
//! adjacent. Tracks Relationships fondness over a short window to verify
//! the kinship bond strengthens via shared dispositions
//! (Caretaking, Socializing).
//!
//! This scenario doubles as a baseline for "should mother-kitten fondness
//! strengthen during normal play?" — if the relationship doesn't grow,
//! kinship-driven scoring downstream (Caretake, Mate, etc.) starves.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::resources::Relationships;

use super::env::{init_scenario_world, spawn_cat, spawn_kitten};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "fondness_kitten_imprint",
    default_focal: "Mother",
    default_ticks: 20,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    let mother = spawn_cat(
        world,
        CatPreset::adult("Mother", Position::new(20, 20))
            .with_personality(|p| {
                p.warmth = 0.85;
                p.compassion = 0.85;
                p.sociability = 0.7;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult),
    );

    let father = spawn_cat(
        world,
        CatPreset::adult("Father", Position::new(22, 20))
            .with_personality(|p| {
                p.warmth = 0.6;
            })
            .with_marker(MarkerKind::Adult),
    );

    let kitten =
        spawn_kitten(
            world,
            CatPreset::kitten("Kit", Position::new(21, 20), current_tick),
            mother,
            father,
        );

    // Mother + father don't know the kitten yet (just born). Initialize
    // pairs so the relationship resource has rows ready to mutate.
    let mut rels = world.remove_resource::<Relationships>().unwrap_or_default();
    {
        let mut rng = world.resource_mut::<crate::resources::SimRng>();
        rels.init_pair(mother, father, &mut rng.rng);
        rels.init_pair(mother, kitten, &mut rng.rng);
        rels.init_pair(father, kitten, &mut rng.rng);
    }
    world.insert_resource(rels);
}
