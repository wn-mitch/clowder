//! Ticket 260 — fox ward-only avoidance scenario.
//!
//! One durable ward + one ShadowFox approaching it, no cats. Proves
//! the magic-perception channel (`WardCoverageMap` read in
//! `wildlife_ai`) still fires after the 260 refactor that moved
//! avoidance off the hardcoded `Ward.repel_radius()` snapshot.
//!
//! Companion to `fox_cat_scent_avoidance`: that scenario covers the
//! scent channel in isolation. Together they demonstrate the two
//! channels are orthogonal substrate — each fires from the channel
//! its substrate corresponds to, not from a shared hardcoded gate.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};

use super::env::init_scenario_world;
use super::Scenario;

const WARD_POS: Position = Position { x: 15, y: 20 };
const FOX_START: Position = Position { x: 35, y: 20 };

pub static SCENARIO: Scenario = Scenario {
    name: "fox_ward_only_avoidance",
    // No cats spawned, so default_focal is informational only — the
    // scenario harness still threads it through the runner config
    // but no focal trace will resolve. Pick a placeholder name.
    default_focal: "Pyre",
    default_ticks: 60,
    setup,
    expected_features: &["ShadowFoxAvoidedWard"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Ticket 023 Phase B: disable the shadow-fox motivation tick so
    // this scenario tests the pre-023 ward-channel contract.
    // Otherwise the motivation softmax may pick Reconstituting (toward
    // the highest-corruption tile in scan), bypassing the patrol-step
    // branch where `ShadowFoxAvoidedWard` fires.
    {
        let mut constants =
            world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        constants.wildlife.shadow_fox_motivation_tick_cadence = u64::MAX;
    }

    // Durable ward — repel_radius ≈ 9 tiles, stamps a meaningful
    // `WardCoverageMap` gradient that crosses the fox's path.
    world.spawn((crate::components::magic::Ward::durable(), WARD_POS));

    // ShadowFox heading west toward the warded tile. The
    // `update_ward_coverage_map` system runs each tick to populate
    // the coverage grid; `wildlife_ai`'s 260 read fires when the
    // fox's next-step coverage crosses `shadow_fox_ward_avoid_threshold`.
    // Ticket 023 Phase A: `ShadowFoxDrives` is the canonical marker
    // for the shadow-fox-only branches; without it the entity bypasses
    // the 260 magic-channel and the avoidance feature never records.
    world.spawn((
        WildAnimal::new(WildSpecies::ShadowFox),
        FOX_START,
        crate::components::physical::Health::default(),
        WildlifeAiState::Patrolling { dx: -1, dy: 0 },
        crate::components::wildlife::ShadowFoxDrives::newly_manifested(0.9),
        crate::components::SensorySpecies::Wild(WildSpecies::ShadowFox),
        crate::components::SensorySignature::WILDLIFE,
    ));
}
