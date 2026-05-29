//! Ticket 246 — non-repro scenario for the colony-scale PickUp lock.
//!
//! Symptom seen in `logs/tuned-42-post-246-floor-removed-collapsed`
//! (post-246 soak with the strict-floor preempt patch removed): 13863
//! PickingUp plan creations + 13568 ItemDropped events across 8 cats ×
//! 5580 ticks. 99.5% of all CatSnapshot actions = PickUp. 0
//! BuildingConstructed. 1172 Resting GoalUnreachable (no Stores → no
//! RestingSpot zone). The 246 plan's "cliff does NOT recur" prediction
//! was wrong; the floor was restored and the follow-on (TBD) owns the
//! diagnosis.
//!
//! This scenario sets up: 3 cats clustered, 5 ground items, no Stores,
//! no Den, no Kitchen. Run for 60 ticks. The lock does **not**
//! manifest at scenario scale — colony-scale dynamics (cat density,
//! plan-failure cascades, no economy bootstrap) are required. The
//! scenario stays as a positive guard: future regressions that DO lock
//! at scenario scale will fail this assertion.

use bevy_ecs::world::World;

use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::physical::{Needs, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

fn spawn_ground_food(world: &mut World, kind: ItemKind, pos: Position) {
    world.spawn((Item::new(kind, 1.0, ItemLocation::OnGround), pos));
}

fn set_focal_hungry(world: &mut World, focal_name: &str) {
    use crate::components::identity::Name;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist");
    let mut em = world.entity_mut(entity);
    let mut needs = em.get_mut::<Needs>().expect("focal has Needs");
    needs.hunger = 0.2;
}

fn assert_has_ground_carcass(world: &mut World) {
    let colony = world
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<crate::components::markers::ColonyState>>()
        .iter(world)
        .next()
        .expect("ColonyState singleton must exist");
    world
        .entity_mut(colony)
        .insert(crate::components::markers::HasGroundCarcass);
}

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    // 3 cats clustered around the colony center, mimicking the soak's
    // cat-density pattern.
    spawn_cat(
        world,
        CatPreset::adult("Cinder", Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );
    spawn_cat(
        world,
        CatPreset::adult("Mallow", Position::new(21, 20)).with_marker(MarkerKind::Adult),
    );
    spawn_cat(
        world,
        CatPreset::adult("Wren", Position::new(20, 21)).with_marker(MarkerKind::Adult),
    );
    set_focal_hungry(world, "Cinder");
    // 5 ground food items in a tight cluster — enough that even with
    // 3 cats picking up, the supply doesn't immediately exhaust on
    // tick 1 (gives the lock window to manifest).
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(22, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 22));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(18, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 18));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(22, 22));
    assert_has_ground_carcass(world);
    // Deliberately NO Stores, NO Den, NO Kitchen — mimics the soak's
    // failure mode where the colony economy never bootstrapped.
}

pub static SCENARIO: Scenario = Scenario {
    name: "intention_momentum_pickup_lock",
    default_focal: "Cinder",
    default_ticks: 60,
    setup,
    expected_features: &[],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// Repro the colony-scale lock pattern: with no Stores to deposit
    /// at and ground items present, the focal cat should NOT spend
    /// over 70% of ticks executing PickUp. If this assertion fires,
    /// the lock is reproducible at scenario scale and the wiring (or
    /// floor-removal) is the cause.
    #[test]
    fn focal_does_not_lock_on_pickup() {
        let report = run(&SCENARIO, None, Some(60), 42);
        let counts = report.winner_counts();
        let total_winners: usize = counts.values().sum();
        let pickup_wins = counts.get("PickUp").copied().unwrap_or(0);
        let pct = (pickup_wins * 100).checked_div(total_winners).unwrap_or(0);
        assert!(
            pct < 70,
            "Focal cat locked on PickUp ({pickup_wins}/{total_winners} = {pct}%); \
             expected diverse activity. winner_counts: {counts:?}",
        );
    }

    /// Post-shuffle-fix invariant: with no Stores reachable, PickingUp's
    /// `HasFoodStorageAccessible` eligibility must reject every cat and
    /// PickUp must win L3 zero times across the scenario. This directly
    /// guards against a regression that re-opens the early-game shuffle
    /// (no destination → no point picking food off the ground).
    #[test]
    fn no_stores_no_pickup_elections() {
        let report = run(&SCENARIO, None, Some(60), 42);
        let counts = report.winner_counts();
        let pickup_wins = counts.get("PickUp").copied().unwrap_or(0);
        assert_eq!(
            pickup_wins, 0,
            "PickUp must not elect when no Stores is reachable \
             (HasFoodStorageAccessible eligibility gate). \
             winner_counts: {counts:?}",
        );
    }

    /// Companion to `no_stores_no_pickup_elections`: with no Stores,
    /// PickUp must not even enter the L2 eligible pool (the eligibility
    /// filter rejects upstream of scoring). Catches regressions where
    /// the marker authoring drifts but PickUp's score is suppressed
    /// some other way (e.g. via a score modifier instead of the gate).
    #[test]
    fn no_stores_pickup_is_ineligible_at_l2() {
        let report = run(&SCENARIO, None, Some(60), 42);
        let any_eligible = report
            .ticks
            .iter()
            .flat_map(|t| t.l2.iter())
            .any(|row| row.dse == "pick_up" && row.eligible);
        assert!(
            !any_eligible,
            "pick_up must be ineligible at L2 across every tick when no \
             Stores is reachable (HasFoodStorageAccessible gate). \
             A regression here means the eligibility check moved out \
             of the filter and back into score-shaping.",
        );
    }
}
