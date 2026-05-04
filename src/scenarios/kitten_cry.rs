//! Kitten-cry triage scenario — the canonical test for the cry-broadcast
//! architecture (ticket 156). One hungry kitten cries; three adults at
//! varying distances and personalities are asked: who picks Caretaking?
//!
//! Expected outcome on a healthy build: **Mallow** (warm + compassionate,
//! adjacent to the kitten) chooses Caretaking on tick 1. The other two
//! adults pick non-caretake dispositions — `Pyre` (independent, far) goes
//! to Resting/Wander; `Dusk` (low-energy, sleeping-default) holds
//! Resting.

use bevy_ecs::world::World;

use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat, spawn_kitten};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "kitten_cry_basic",
    default_focal: "Mallow",
    // 5 ticks: enough for tick-1 disposition pick + ~4 ticks of commitment
    // hysteresis so we can see "Mallow locks Caretake while others pivot".
    default_ticks: 5,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Center the cast tightly around (20, 20) so the cry-map's ~12-tile
    // sense range covers everyone. Kitten-cry-hunger threshold defaults
    // to 0.6; setting hunger=0.2 produces ~67% strength at the source.
    let kitten_pos = Position::new(20, 20);
    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    // Mother (Mallow): warm + compassionate, adjacent — should pick
    // Caretaking. Marked as Parent + IsParentOfHungryKitten so both the
    // spatial and kinship channels of the cry-broadcast architecture
    // fire.
    let mallow = spawn_cat(
        world,
        CatPreset::adult("Mallow", Position::new(21, 20))
            .with_personality(|p| {
                p.warmth = 0.9;
                p.compassion = 0.9;
                p.sociability = 0.7;
                p.diligence = 0.7;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::IsParentOfHungryKitten)
            .with_marker(MarkerKind::Adult),
    );

    // Father-figure (Pyre): independent, far — should NOT prioritize
    // Caretake. At distance 10 the cry-map signal is attenuated to ~17%
    // of source strength.
    let _pyre = spawn_cat(
        world,
        CatPreset::adult("Pyre", Position::new(30, 20))
            .with_personality(|p| {
                p.independence = 0.9;
                p.warmth = 0.2;
                p.compassion = 0.2;
                p.sociability = 0.3;
            })
            .with_marker(MarkerKind::Adult),
    );

    // Tired adult (Dusk): low energy → Resting wins regardless of cry.
    // Tests the "Maslow physiological pre-empts higher levels" path.
    let _dusk = spawn_cat(
        world,
        CatPreset::adult("Dusk", Position::new(19, 21))
            .with_personality(|p| {
                p.warmth = 0.6;
                p.compassion = 0.6;
            })
            .with_needs(|n| {
                n.energy = 0.05;
            })
            .with_marker(MarkerKind::Adult),
    );

    // The hungry kitten — partner is set to Pyre to give the kinship
    // channel a non-Mallow second parent in the dependency record.
    let _kitten = spawn_kitten(
        world,
        CatPreset::kitten("Crumb", kitten_pos, current_tick).with_needs(|n| {
            // Below kitten_cry_hunger_threshold (default 0.6) so the
            // cry-map stamps a non-zero signal. 0.2 puts the source
            // strength at (0.6 - 0.2) / 0.6 ≈ 0.67.
            n.hunger = 0.2;
        }),
        mallow, // mother
        _pyre,  // father
    );
}
