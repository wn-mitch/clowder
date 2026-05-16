//! Ticket 230 microexperiment — substrate-aware flee commitment.
//!
//! A wounded cat with a wildlife threat at adjacent range elects
//! `Action::Flee` via the `ThreatProximityAdrenalineFlee` modifier
//! lift (post-251 — `AcuteHealthAdrenalineFlee` was retired with the
//! load shifted to Sleep's `health_deficit` Logistic axis, so injury
//! alone no longer lifts Flee). The new `DispositionKind::Fleeing`
//! plan template `[PickFleeTarget, Flee, HoldUntilSafe]` then
//! dispatches.
//! `PickFleeTarget` reads the per-replan `RouteCostField` (boldness-
//! scaled fox-scent overlay) and writes the lowest-cost passable tile
//! within `flee_distance` to `target_position`. The umbrella `Flee`
//! travel step walks the cat there via `cat_path_plan!`; the
//! `HoldUntilSafe` step counter-downs `flee_hold_ticks` while the cat
//! sits on a low-cost tile with `safety_need ≥ flee_safety_need_threshold`.
//!
//! Demonstrative invariants asserted at scenario completion:
//!   - `Feature::FleeTargetPicked` fires ≥ 1× — the substrate-aware
//!     picker is reachable end-to-end (gated by `expected_features`
//!     after 254 R5 rebound the witness contract from `cost <
//!     current_cost` to effective-cost minimization).
//!   - The `try_preempt_with_modifier_lurch` early-skip composes with
//!     `Fleeing`: while the cat is in a Fleeing plan, the
//!     adrenaline modifiers' `preempts_in_flight` signal is ignored,
//!     so the cat accumulates hold ticks instead of thrashing
//!     `PickFleeTarget` on every modifier ramp tick (the post-228
//!     thrash cadence was 1.21 ticks/plan; post-230 should be
//!     ≥ flee_hold_ticks ticks/plan during the active flee window).
//!
//! Setup: a wounded cat (health = 0.4 — exercises the Sleep axis's
//! Logistic urgency post-251) plus a fox at adjacent range to lift
//! `ThreatProximityAdrenalineFlee`.

use bevy_ecs::world::World;

use crate::components::physical::{Health, Position};
use crate::components::wildlife::WildAnimal;
use crate::resources::FoxScentMap;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "flee_commitment",
    default_focal: "Brave",
    default_ticks: 60,
    setup,
    // 254 R5 closed the witness contract — `PickFleeTarget` now
    // minimizes `effective_cost = field.cost_at(candidate) - chebyshev(candidate, threat)`
    // instead of `cost < current_cost` (which was unreachable because
    // `flood_dijkstra` hardcodes `cost_at(origin) = 0`). The picker
    // emits whenever any reachable, passable, non-self tile exists in
    // the disc, so `Feature::FleeTargetPicked` fires end-to-end on the
    // focal cat's first Fleeing tick.
    expected_features: &["FleeTargetPicked"],
};

pub const FOCAL_START: Position = Position { x: 20, y: 20 };
pub const FOX_POS: Position = Position { x: 22, y: 20 };
/// Substrate-blind naive vector-projection picker (the pre-230 code
/// path) would project the cat away from the fox along (-2, 0)
/// scaled by `flee_distance = 8`, landing near (12, 20). Saturating
/// fox-scent at x = 14 ensures that direction is ALSO an expensive
/// corridor — the test that the substrate-aware picker actually
/// reads `RouteCostField` rather than projecting blindly.
pub const NAIVE_CORRIDOR_X: i32 = 14;

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Saturate the naive-projection corridor with fox-scent so the
    // substrate-aware picker is forced to reject the simple "away
    // from threat" tile in favor of a route-cost-cheaper detour.
    {
        let mut fox = world.resource_mut::<FoxScentMap>();
        for y in 16..=24i32 {
            fox.deposit(NAIVE_CORRIDOR_X, y, 1.0);
        }
    }

    // Spawn the focal cat — wounded enough to saturate Sleep's
    // post-251 Logistic `health_deficit` axis (deficit ≥ 0.5 at
    // health = 0.4 and midpoint = 0.4 + transition-width ≈ 0.1).
    // Bold so the boldness-scaled overlay weight in the per-cat
    // RouteCostField flood is mild — the substrate-aware picker
    // still has cheap tiles available within `flee_distance` Chebyshev.
    let cat = spawn_cat(
        world,
        CatPreset::adult("Brave", FOCAL_START)
            .with_personality(|p| {
                p.boldness = 0.7;
                p.diligence = 0.5;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                // Mid-low safety so threat-proximity-derivative has
                // headroom to ramp on the fox spawn.
                n.safety = 0.3;
                n.hunger = 0.7;
                n.energy = 0.8;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Wound the cat to deficit 0.6 so Sleep's post-251 Logistic
    // `health_deficit` axis saturates above the midpoint.
    world.entity_mut(cat).insert(Health {
        current: 0.4,
        max: 1.0,
        total_starvation_damage: 0.0,
    });

    // Spawn a fox at adjacent range so the threat-proximity scalar
    // ramps and Flee gets lifted by both adrenaline modifiers.
    world.spawn((
        FOX_POS,
        WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
    ));
}

// 252: L3 election now reachable (the legacy `Action::Flee` filter
// at `scoring.rs:2411` was lifted as a substrate-stub regression
// fix). End-to-end gating defers until the PickFleeTarget witness
// contract is rebound — the picker compares `cost < current_cost`
// but `flood_dijkstra` always sets `cost_at(origin) = 0`, so the
// picker can't witness on a fresh-flood plan. Tracked as a 252
// follow-on ticket. Substrate-aware target-picker contract coverage
// lives in `src/steps/disposition/pick_flee_target.rs`'s unit tests;
// hold-until-safe contract coverage lives in
// `src/steps/disposition/hold_until_safe.rs`. End-to-end
// commitment-cadence verification (the 39,536× → < 4,000×
// preempt-rate target named in 230's `## Verification` section)
// continues to live at the soak level.
