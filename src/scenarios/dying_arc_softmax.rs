//! Ticket 232 microexperiment — body-state-coupled L3 softmax temperature.
//!
//! Dying-arc replay setup: wounded cat (HP = 0.49, saturating
//! `health_deficit` and so `body_distress_composite ≥ 0.49`) with a
//! fox at adjacent range and pre-lowered `safety` need so
//! `threat_proximity_derivative` ramps once the fox spawns. With both
//! perception scalars high, [`crate::ai::scoring::softmax_temperature`]
//! returns the floor (T = 0.05 by default), sharpening the L3 draw.
//!
//! # Why this is a triage harness, not a fix-lock
//!
//! The mathematical shape that lands the dying-arc fix is two-part:
//!
//! 1. **Ticket 231** widens the L2 gap by subscribing pickup-class
//!    DSEs to body-state Considerations — under body distress, Sleep
//!    scores meaningfully above PickUp.
//! 2. **Ticket 232** (this ticket) sharpens the L3 softmax around
//!    that wider gap so the cat picks the higher-scoring DSE
//!    deterministically.
//!
//! 232 alone, against the original 1% L2 margin (PickUp 0.958, Flee
//! 0.948 in `logs/tuned-42` Calcifer's fatal tick), only lifts the
//! winner's probability from ~52% to ~55% — sharper, but not
//! decisive. The decisive behavior emerges only with 231's L2
//! widening composed in.
//!
//! Therefore this scenario ships with `expected_features: &[]` and is
//! documented as a per-tick **triage harness**. Run it post-231-land
//! to manually inspect the per-tick winning DSE and confirm the L3
//! draw is no longer a coin flip. The load-bearing temperature
//! assertions live in the unit tests:
//!
//! - `softmax_temperature_at_floor_under_dying_arc`
//! - `floor_temperature_sharpens_against_one_percent_margin`
//! - `floor_temperature_is_decisive_against_widened_margin`
//! - `ceiling_temperature_keeps_one_percent_margin_stochastic`
//!
//! Setup mirrors `flee_commitment` (wounded + adjacent fox) so the
//! two scenarios diff cleanly: `flee_commitment` exercises the
//! Fleeing-chain pipeline; this one exercises the L3 temperature
//! sharpness against the same perception state.

use bevy_ecs::world::World;

use crate::components::physical::{Health, Position};
use crate::components::wildlife::WildAnimal;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "dying_arc_softmax",
    default_focal: "Calcifer",
    default_ticks: 30,
    setup,
    // 232: see module doc — fix-lock requires 231 + 232 together.
    // This scenario is a triage harness; load-bearing assertions
    // live in the scoring unit tests.
    expected_features: &[],
};

pub const FOCAL_START: Position = Position { x: 20, y: 20 };
pub const FOX_POS: Position = Position { x: 22, y: 20 };

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Focal cat: wounded enough to saturate `body_distress_composite`
    // (health_deficit dominates the max-of-axes composition), with a
    // pre-lowered safety need so `threat_proximity_derivative` ramps
    // when the fox is spawned in the same tick. Both perception
    // scalars rise → `softmax_temperature` returns the floor.
    let cat = spawn_cat(
        world,
        CatPreset::adult("Calcifer", FOCAL_START)
            .with_personality(|p| {
                p.boldness = 0.5;
                p.diligence = 0.5;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                n.safety = 0.3;
                n.hunger = 0.7;
                n.energy = 0.8;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // HP = 0.49 reproduces the canonical Calcifer fatal-tick state.
    // body_distress_composite = max(…, health_deficit = 0.51) ≥ 0.51.
    world.entity_mut(cat).insert(Health {
        current: 0.49,
        max: 1.0,
        injuries: Vec::new(),
        total_starvation_damage: 0.0,
    });

    // Fox at adjacent range — same shape as `flee_commitment`. Drives
    // the threat-proximity-derivative scalar up on the next tick.
    world.spawn((
        FOX_POS,
        WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
    ));
}
