//! Ticket 228 microexperiment — bold-vs-timid route-cost suppression.
//!
//! Two cats with identical needs but opposite boldness are placed on
//! one side of a fox-scent corridor; a single prey sits on the far
//! side. With `hunt_route_cost_weight = 1.0`, the L2 Hunt axis reads
//! the cat's `RouteCostField` at the prey landmark and converts the
//! cost-to-reach into a (closer-is-better) score. Bold cats flood
//! with low overlay weight (`cat_path_weight_from_boldness(0.9)` ≈
//! 0.1) so the corridor barely raises their reach cost; timid cats
//! flood with full weight (`≈ 0.9`) so the corridor's per-tile cost
//! adds up across the column.
//!
//! Demonstrative invariants (asserted in
//! `tests/route_cost_decision.rs`):
//!   - `bold.cost_at(prey_pos) < timid.cost_at(prey_pos)` after the
//!     first replan (the L1 substrate signal).
//!   - `bold.l2_hunt_score > timid.l2_hunt_score` for the same prey
//!     anchor (the L2 conversion that 228's `Consideration::Field`
//!     evaluator performs).
//!
//! The scenario itself runs with "Bold" as the focal so `just scenario
//! route_cost_decision` shows Bold's trace picking Hunt despite the
//! corridor; the test runs both focals and compares.
//!
//! `hunt_route_cost_weight` ships at 0.0 in canonical sim constants
//! (substrate-dormant); the scenario lifts it to 1.0 locally on the
//! world's `SimConstants` resource so the substrate exercises. Soak
//! footprint is unchanged because soak doesn't run scenarios.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::PreyKind;
use crate::resources::{FoxScentMap, SimConstants};

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "route_cost_decision",
    default_focal: "Bold",
    default_ticks: 4,
    setup,
    expected_features: &[],
};

/// World coordinates the assertion test reads back.
pub const BOLD_START: Position = Position { x: 5, y: 20 };
pub const TIMID_START: Position = Position { x: 5, y: 22 };
pub const PREY_POS: Position = Position { x: 35, y: 20 };
/// Fox-scent corridor — one bucket column. With the default
/// `bucket_size = 5`, depositing at any tile in the bucket fills the
/// whole 5×5 region. We saturate every bucket along x ∈ [15, 19] so
/// the corridor spans the full y-axis the cats might detour through.
pub const CORRIDOR_X: i32 = 17;

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Lift the Hunt route-cost axis to 1.0 so the substrate exercises.
    // Canonical sim constants ship at 0.0 (substrate-dormant); this
    // override is local to the scenario world.
    {
        let mut constants = world.resource_mut::<SimConstants>();
        constants.scoring.hunt_route_cost_weight = 1.0;
    }

    // Saturate the fox-scent corridor. The bucket grid covers the
    // 40×40 scenario map at bucket_size = 5, so depositing at one
    // representative tile per bucket suffices to fill the bucket.
    {
        let mut fox = world.resource_mut::<FoxScentMap>();
        for y in 0..40i32 {
            // Saturate by depositing 1.0 — `deposit` clamps to 1.0.
            fox.deposit(CORRIDOR_X, y, 1.0);
        }
    }

    // Bold: low boldness-conditioned overlay weight ⇒ corridor barely
    // raises reach cost.
    let _bold = spawn_cat(
        world,
        CatPreset::adult("Bold", BOLD_START)
            .with_personality(|p| {
                p.boldness = 0.9;
                p.diligence = 0.7;
                p.patience = 0.7;
            })
            .with_needs(|n| {
                n.hunger = 0.45;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Timid: high overlay weight ⇒ corridor is expensive, route-cost
    // to prey saturates near MAX_COST_BUDGET, Hunt L2 score collapses.
    let _timid = spawn_cat(
        world,
        CatPreset::adult("Timid", TIMID_START)
            .with_personality(|p| {
                p.boldness = 0.1;
                p.diligence = 0.7;
                p.patience = 0.7;
            })
            .with_needs(|n| {
                n.hunger = 0.45;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Single mouse on the far side of the corridor.
    let _mouse = spawn_prey_at(world, PREY_POS, PreyKind::Mouse);
}
