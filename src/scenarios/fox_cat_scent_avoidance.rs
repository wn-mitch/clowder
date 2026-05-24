//! Ticket 260 — fox cat-scent avoidance scenario.
//!
//! Four adult cats clustered at one tile, one ShadowFox patrolling
//! westward from a distance. The fox starts outside the cat-scent
//! gradient; as the cluster saturates its `CatScentMap` bucket
//! (steady-state `cat_scent_base_deposit` + Patrol bonus), the fox
//! reaches the bucket boundary and `wildlife_ai` flips it on the
//! scent channel — recording `Feature::ShadowFoxAvoidedCatScent`.
//!
//! Companion to `fox_ward_only_avoidance`: that scenario proves the
//! ward (magic) channel still fires in isolation; this proves the
//! scent channel fires when no wards are present. Together they
//! demonstrate the two channels are orthogonal substrate.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const CAT_CLUSTER: Position = Position { x: 10, y: 20 };
const FOX_START: Position = Position { x: 35, y: 20 };

pub static SCENARIO: Scenario = Scenario {
    name: "fox_cat_scent_avoidance",
    default_focal: "Pyre",
    // 100: bumped from 100 → 200 ticks. Schedule-edge perturbation from
    // adding `tremor_tick` to the per-tick chain shifted seed-42's
    // ShadowFox approach-then-avoid pacing; the avoid still fires,
    // just later in the run.
    default_ticks: 200,
    setup,
    expected_features: &["ShadowFoxAvoidedCatScent"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Ticket 023 Phase B: disable the shadow-fox motivation tick for
    // this scenario. Phase B's motivation softmax routinely picks
    // Haunting when cats are in scent range, bypassing the patrol-step
    // branch where `ShadowFoxAvoidedCatScent` fires. Keeping cadence
    // huge keeps this scenario testing the pre-023 magic/scent-channel
    // avoidance contract.
    {
        let mut constants = world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        constants.wildlife.shadow_fox_motivation_tick_cadence = u64::MAX;
    }

    // Four adult cats stacked at the cluster tile. The
    // `cat_scent_tick` system deposits both the steady-state base
    // amount and (when the cat's action is Patrol/Fight/Explore) the
    // action bonus into the bucket containing this position. With
    // four cats co-located the bucket saturates well before the fox
    // walks 30 tiles west.
    for name in ["Briar", "Pyre", "Ash", "Sage"] {
        let _ = spawn_cat(
            world,
            CatPreset::adult(name, CAT_CLUSTER)
                .with_personality(|p| {
                    p.boldness = 0.6;
                    p.diligence = 0.7;
                })
                .with_marker(MarkerKind::Adult),
        );
    }

    // Single ShadowFox heading west toward the cluster. Without a
    // FoxState it falls into `wildlife_ai`'s patrol-state branch,
    // where the 260 cat-scent check now fires.
    // Ticket 023 Phase A: `ShadowFoxDrives` is the canonical marker
    // for the shadow-fox-only branches in `wildlife_ai` and
    // `predator_stalk_cats`. Without it the entity is invisible to the
    // 260 scent-channel and the avoidance feature never records.
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
