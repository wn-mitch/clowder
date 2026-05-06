//! Ticket 193 — election-side scenario for the re-routed PickingUp
//! plan template. A focal cat with an empty inventory and no stored
//! food encounters three OnGround food `Item` entities; the
//! `HasGroundCarcass` colony marker (re-wired to gate on the food-
//! Item surface) makes PickingUp eligible, the inverted-Logistic
//! `colony_food_security` curve scores high, and the new
//! `PlannerZone::CarcassPile` resolves to a real position so A*
//! produces a viable plan.
//!
//! Pre-193 baseline: the plan template routed through
//! `PlannerZone::MaterialPile`, which filtered to build materials
//! only — A* found no target, the plan failed with `GoalUnreachable`,
//! and the cat replanned every tick. The seed-42 canonical soak
//! recorded 1367 such failures per 10kt, driving colony collapse.
//!
//! Pass criteria:
//! - `Action::PickUp` wins L3 election at least once within the tick
//!   budget, AND
//! - the run completes without panicking on any zone-resolution path
//!   (proves `CarcassPile` is wired everywhere `MaterialPile` was).

use bevy_ecs::world::World;

use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::physical::{Needs, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position { x: 20, y: 20 };

/// Spawn a single OnGround food `Item` at `pos`. Mirrors the
/// engage_prey-overflow spawn at `goap.rs::resolve_engage_prey`
/// (the production source of these entities).
fn spawn_ground_food(world: &mut World, kind: ItemKind, pos: Position) {
    world.spawn((Item::new(kind, 1.0, ItemLocation::OnGround), pos));
}

/// Pre-insert `HasGroundCarcass` on the colony singleton so tick 1's
/// scoring pass sees it. `update_colony_building_markers` re-asserts
/// the marker each tick once it observes the spawned Items, but it
/// runs *after* `evaluate_and_plan` on the first tick — same race-
/// guard pattern as `disposal_election::assert_has_midden`.
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

/// Set the focal cat's hunger low so `colony_food_security`
/// (`min(food_fraction, hunger_satisfaction)`) is low and the
/// inverted-Logistic curve gives PickingUp a high score. Empty
/// `FoodStores` already drives `food_fraction` to 0, but the L2
/// composite is the min of the two; making both small keeps the
/// curve far from any near-1 corner.
fn set_focal_hungry(world: &mut World, focal_name: &str) {
    use crate::components::identity::Name;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist before set_focal_hungry");
    let mut em = world.entity_mut(entity);
    let mut needs = em.get_mut::<Needs>().expect("focal has Needs");
    needs.hunger = 0.2;
}

fn setup_picking_up_scavenging(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    set_focal_hungry(world, "Cinder");
    // Three RawMouse Items within an L2 walking step of the cat. The
    // CarcassPile zone resolves to the nearest entity by manhattan
    // distance; A* threads a TravelTo(CarcassPile) step ahead of the
    // PickUpItemFromGround. We don't assert which one the cat picks —
    // only that the plan resolves without GoalUnreachable.
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(22, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 22));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(18, 20));
    assert_has_ground_carcass(world);
}

pub static SCENARIO: Scenario = Scenario {
    name: "picking_up_scavenging",
    default_focal: "Cinder",
    // Empirically tick 1 commits to Wander (a multi-tick fallback plan)
    // before PickingUp's eligibility flows through the L1→L2 pool. The
    // commit suppresses L3 emission for several ticks; the first
    // PickUp election lands around tick 11 in the ~12-tick range. Set
    // to 16 to give the test budget enough headroom to capture ≥1
    // PickUp win across seed-42 noise.
    default_ticks: 16,
    setup: setup_picking_up_scavenging,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// With three OnGround food-Items present, low cat hunger, and
    /// empty FoodStores, the `colony_food_security` axis sits near 0
    /// and the inverted Logistic gives PickingUp a near-1 score.
    /// Eligibility passes via the pre-inserted `HasGroundCarcass`
    /// marker; PickingUp wins L3 at least once across the budget.
    /// (Other low-tier needs may also fire — Hunting/Foraging — so
    /// the assertion is "at least one win", not "every tick wins".)
    #[test]
    fn picking_up_wins_with_ground_food_present() {
        let report = run(&SCENARIO, None, Some(16), 42);
        let counts = report.winner_counts();
        let pickup_wins = counts.get("PickUp").copied().unwrap_or(0);
        assert!(
            pickup_wins >= 1,
            "PickingUp (Action::PickUp) should win L3 at least once with three OnGround food-Items, \
             a hungry cat, and empty FoodStores; got {counts:?}",
        );
    }

    /// End-state regression check: at run end, the cat's inventory has
    /// gained at least one slot (proves the resolver actually picked
    /// up an Item) AND the ground-item count dropped (proves the
    /// despawn happened). Catches the pre-193 failure mode where
    /// PickingUp would win L3 but A* failed `GoalUnreachable` and the
    /// resolver never executed.
    #[test]
    fn pick_up_resolver_actually_executes() {
        let report = run(&SCENARIO, None, Some(16), 42);
        assert!(
            report.final_focal_inventory_count >= 1,
            "focal cat must end with ≥1 inventory slot used (proves PickUp resolver ran); \
             got {} slots",
            report.final_focal_inventory_count,
        );
        assert!(
            report.final_ground_item_count <= 2,
            "ground item count must drop from 3 to ≤2 (proves Item entity despawned on pickup); \
             got {} ground items remaining",
            report.final_ground_item_count,
        );
    }
}
