//! Ward placement scenario — an adult cat carrying thornbriar herbs near
//! a corrupted tile. Tests the herbcraft_ward DSE eligibility chain
//! (`CanWard` marker depends on `HasWardHerbs` which is authored from
//! inventory in `update_inventory_markers`).
//!
//! The harness gives the cat thornbriar items at spawn; the
//! `update_inventory_markers` system runs early in Chain 2a and authors
//! `HasWardHerbs`; `update_capability_markers` then authors `CanWard`;
//! the herbcraft_ward DSE becomes eligible.
//!
//! 155: post-Crafting-split, the focal cat now picks the new
//! `Herbalism` Disposition with `Action::HerbcraftSetWard` as the
//! chosen sub-action. The plan template branches on the sub-action
//! and emits the gather-thornbriar → set-ward chain.
//!
//! Expected on a healthy build: focal cat picks Herbalism (HerbcraftSetWard
//! sub-action) within a few ticks once the marker chain stabilizes. If
//! the ward is never placed, the scenario surfaces *which* marker /
//! step in the chain stalls.

use bevy_ecs::world::World;

use crate::components::magic::HerbKind;
use crate::components::physical::Position;

use super::env::{give_herbs, init_scenario_world, mark_tile_corrupted, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "ward_placement",
    default_focal: "Sage",
    default_ticks: 40,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Corruption hot-spot 4 tiles east — gives the ward DSE somewhere
    // meaningful to score against.
    mark_tile_corrupted(world, Position::new(24, 20), 0.7);
    // Surrounding tiles also slightly corrupted so ward_strength_low
    // averages high.
    for dx in -2..=2 {
        for dy in -2..=2 {
            mark_tile_corrupted(world, Position::new(24 + dx, 20 + dy), 0.4);
        }
    }

    let sage = spawn_cat(
        world,
        CatPreset::adult("Sage", Position::new(20, 20))
            .with_personality(|p| {
                p.spirituality = 0.85;
                p.diligence = 0.7;
                p.compassion = 0.7;
            })
            .with_magic_affinity(0.6)
            .with_marker(MarkerKind::Adult),
    );

    // Hand Sage thornbriar — `update_inventory_markers` (Chain 2a) will
    // author `HasWardHerbs` on the next tick, then
    // `update_capability_markers` adds `CanWard`.
    give_herbs(world, sage, HerbKind::Thornbriar, 3);
}
