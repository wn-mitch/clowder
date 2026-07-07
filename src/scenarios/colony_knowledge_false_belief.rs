//! Colony-knowledge false-belief scenario — ticket 291.
//!
//! Three cats are preloaded with the SAME wrong belief: strong
//! `recency_of_threat_cue` about a perfectly safe meadow bucket no
//! threat has ever touched. A fourth cat holds a *divergent* strong
//! belief about a second bucket (values spread past
//! `agreement_epsilon` against two calm-ish holders), which must NOT
//! promote.
//!
//! What this pins, at full-app integration (real schedule, real
//! `belief_integrator` decay running alongside):
//!
//! 1. **Substrate does not gate truth** — the false consensus
//!    promotes to `ColonyKnowledge` (the load-bearing C3 narrative
//!    the carrier-count model precluded: panic can propagate faster
//!    than ground truth corrects).
//! 2. **The witness chain is citable** — the promoted entry lists
//!    the three believers.
//! 3. **Divergence is not consensus** — the contested bucket stays
//!    out of colony knowledge and accrues
//!    `divergence_duration_ticks` instead.
//!
//! The assertion test lives in `tests/scenarios.rs`
//! (`colony_knowledge_false_belief_promotes_with_witness_chain`) and
//! inspects the `ColonyKnowledge` resource directly via
//! `runner::build_scenario_app`.

use bevy_ecs::world::World;

use crate::components::beliefs::{bucket_position, Facet, LocationBeliefs};
use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

/// The safe meadow the three believers wrongly agree is dangerous.
pub const FALSE_THREAT_POS: (i32, i32) = (30, 30);
/// The contested bucket (one alarmed cat vs two calm ones).
pub const CONTESTED_POS: (i32, i32) = (10, 30);

pub static SCENARIO: Scenario = Scenario {
    name: "colony_knowledge_false_belief",
    default_focal: "Rumor",
    // Ticks run from start_tick+1, so the first derivation scan
    // (every scan_interval=500) lands at +500; 510 ticks crosses it
    // exactly once. Planted strengths (0.9) survive ~25 Pass-B decay
    // steps comfortably above promotion_strength (0.3).
    default_ticks: 510,
    setup,
    expected_features: &["KnowledgePromoted"],
};

fn plant_threat_belief(
    world: &mut World,
    cat: bevy_ecs::entity::Entity,
    pos: (i32, i32),
    value: f32,
) {
    let mut locs = world
        .get_mut::<LocationBeliefs>(cat)
        .expect("scenario cats carry LocationBeliefs");
    let model = locs
        .models
        .entry(bucket_position(pos.0, pos.1))
        .or_default();
    model.recency_of_threat_cue = Facet {
        value,
        strength: 0.9,
        ..Default::default()
    };
}

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let believers = ["Rumor", "Echo", "Whisper"];
    let mut spawned = Vec::new();
    for (i, name) in believers.iter().enumerate() {
        let cat = spawn_cat(
            world,
            CatPreset::adult(*name, Position::new(20 + i as i32, 20))
                .with_marker(MarkerKind::Adult),
        );
        spawned.push(cat);
    }
    // The false consensus: all three strongly believe the safe meadow
    // is dangerous (same value — well within agreement_epsilon).
    for cat in &spawned {
        plant_threat_belief(world, *cat, FALSE_THREAT_POS, 0.85);
    }

    // The contested bucket: one alarmed cat vs two calm-ish cats —
    // values 0.9 / 0.30 / 0.25 with epsilon 0.2: median 0.30, the 0.9
    // outlier drops, and the agreeing pair {0.30, 0.25} is below the
    // quorum of 3.
    plant_threat_belief(world, spawned[0], CONTESTED_POS, 0.9);
    plant_threat_belief(world, spawned[1], CONTESTED_POS, 0.30);
    plant_threat_belief(world, spawned[2], CONTESTED_POS, 0.25);
}
