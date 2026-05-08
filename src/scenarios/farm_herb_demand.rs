//! Farm DSE herb-pressure scenario (ticket 086) — deterministic
//! exercise of the `Farm` → tend → harvest chain to assert
//! `Feature::CropTended` and `Feature::CropHarvested` fire end-to-end.
//!
//! # Why a scenario, not a soak
//!
//! The canonical seed-42 deep soak does not naturally reach the
//! `(ward_strength_low ∧ !ThornbriarAvailable)` regime: post-Wave-2
//! food production plateaus near full and the colony never builds a
//! garden, so the build-pressure → garden → repurpose chain that 084
//! engineered never executes. Ticket 085's loose-gate probe
//! (`logs/sweep-gap-repro/`) demonstrated that reaching the regime
//! naturally requires constants edits that break other survival
//! canaries. 086's structural reframe locks the integration-test
//! surface here instead — the soak canary stays demoted (correct,
//! given the empirical evidence), and natural-conditions firing is
//! parked pending a sociology/economy reapproach.
//!
//! # Preloaded state
//!
//! - Garden at (22,20) with a Thornbriar `CropState` attached
//!   post-spawn. Initial growth pre-loaded near maturity (0.95) so
//!   the tend cycle finishes in a handful of ticks rather than
//!   ~200 — this scenario tests the *integration* of the tend +
//!   harvest pipeline, not the growth-rate timing.
//!   `spawn_garden_at` emits only the Structure + Position;
//!   CropState is normally inserted by
//!   `steps/building/construct.rs:117` on build completion. We
//!   bypass construction so this scenario tests the tend-and-
//!   harvest path in isolation (the build path lives in
//!   `farming_cycle.rs`).
//! - Zero Wards spawned → `magic::is_ward_strength_low` returns
//!   true for the empty iterator (`magic.rs:30-37`), so
//!   `ctx.ward_strength_low` is true and the colony marker
//!   `WardStrengthLow` is authored.
//! - Zero Thornbriar `Herb` entities → `ThornbriarAvailable` stays
//!   unset → the herb-pressure axis evaluates to 1.0 at scoring
//!   time (`scoring.rs:764-776`), the load-bearing lift that 084
//!   added.
//! - `Bracken`: adult, sated, high diligence. Curiosity and
//!   boldness pinned low so Explore / Wander don't crowd Farm at
//!   the L3 softmax in early ticks (Explore plans hold for
//!   hundreds of ticks once committed). `Skills.foraging = 1.0`
//!   set post-spawn so each tend increment is meaningful against
//!   the Thornbriar 0.5× growth modifier (`tend.rs:87`).
//!
//! # Tick budget
//!
//! Pre-loaded growth = 0.95 + foraging = 1.0 → each tend adds
//! 0.005 → ≤ 10 tends to harvest. Budget 80 leaves wide margin for
//! Farm election, the one-tile walk, and any L3 oscillation in
//! early ticks. Compresses the cycle vs. a "real" colony's
//! ~200-tick maturation; that growth-rate timing is its own knob,
//! not what this scenario gates.

use bevy_ecs::world::World;

use crate::components::building::{CropKind, CropState};
use crate::components::physical::Position;
use crate::components::skills::Skills;

use super::env::{init_scenario_world, spawn_cat, spawn_garden_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "farm_herb_demand",
    default_focal: "Bracken",
    default_ticks: 80,
    setup,
    expected_features: &["CropTended", "CropHarvested"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let garden = spawn_garden_at(world, Position::new(22, 20));
    world.entity_mut(garden).insert(CropState {
        growth: 0.95,
        crop_kind: CropKind::Thornbriar,
    });

    let bracken = spawn_cat(
        world,
        CatPreset::adult("Bracken", Position::new(22, 20))
            .with_personality(|p| {
                p.diligence = 0.95;
                p.patience = 0.85;
                p.tradition = 0.7;
                p.curiosity = 0.05;
                p.boldness = 0.1;
                p.playfulness = 0.1;
            })
            .with_needs(|n| {
                n.hunger = 1.0;
                n.energy = 0.9;
                n.purpose = 0.3;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanForage),
    );

    world.entity_mut(bracken).insert(Skills {
        foraging: 1.0,
        ..Skills::default()
    });
}
