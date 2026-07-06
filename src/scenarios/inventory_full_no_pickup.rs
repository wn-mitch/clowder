//! Ticket 231 — election + plan composition microexperiment for the
//! capacity-aware pickup pipeline.
//!
//! Three sister scenarios demonstrate the dual-branch
//! substrate-vs-plan-path composition introduced in 231:
//!
//! - **`inventory_full_curios`** — cat 5/5 of `ShinyPebble`, adjacent
//!   ground food, hungry. Substrate-path of `PickUpItemFromGround`
//!   blocked (`HasFreeSlot` absent); A* composes `[DropItem, PickUp]`
//!   via the plan-path (`HasFreeSlotThisPlan(true)`). The runtime
//!   resolver's `drop_priority` picks the pebble (base 0.05) over food.
//! - **`inventory_full_herbs`** — same shape with `HerbHealingMoss`
//!   instead of pebbles. Validates the `ItemSlot` collapse: herbs are
//!   no longer skipped by `resolve_drop_item` (pre-231 the resolver
//!   filtered to `ItemSlot::Item` only, leaving herb-clogged cats
//!   permanently stuck).
//! - **`inventory_empty_pickup_unchanged`** — cat 0/5, adjacent food.
//!   Substrate-path fires; plan is `[PickUpItemFromGround]` (no
//!   DropItem prefix). Regression guard for the cheap path.
//!
//! Pass criteria across the bundle:
//! - Full-inventory cases: `Action::Drop` wins L3 at least once AND
//!   `Action::PickUp` wins L3 at least once within the tick budget.
//!   Slot count temporarily decreases (drop), then increases (pickup).
//! - Empty case: `Action::PickUp` wins; `Action::Drop` does NOT.

use bevy_ecs::world::World;

use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::Inventory;
use crate::components::physical::{Needs, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position::new(20, 20);

fn spawn_ground_food(world: &mut World, kind: ItemKind, pos: Position) {
    world.spawn((Item::new(kind, 1.0, ItemLocation::OnGround), pos));
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

/// Spawn a Stores building so PickingUp's `HasFoodStorageAccessible`
/// eligibility gate passes (early-game shuffle fix). All three sister
/// scenarios in this file isolate the inventory-full vs substrate-path
/// election shape; they don't exercise deposit, so the Stores is
/// purely a precondition for the L3 election test.
fn spawn_stores_west(world: &mut World) {
    use crate::components::building::{StoredItems, Structure, StructureType};
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        Position::new(16, 20),
    ));
}

fn fill_focal_inventory(world: &mut World, focal_name: &str, kind: ItemKind, count: usize) {
    use crate::components::identity::Name;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist before filling inventory");
    let mut em = world.entity_mut(entity);
    let mut inv = em.get_mut::<Inventory>().expect("focal has Inventory");
    for _ in 0..count {
        inv.add_item(kind);
    }
}

// ----------------------------------------------------------------------
// inventory_full_curios — full of ShinyPebble, adjacent ground food.
// ----------------------------------------------------------------------

fn setup_full_curios(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    set_focal_hungry(world, "Cinder");
    fill_focal_inventory(world, "Cinder", ItemKind::ShinyPebble, 5);
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(21, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 21));
    spawn_stores_west(world);
    assert_has_ground_carcass(world);
}

pub static SCENARIO_FULL_CURIOS: Scenario = Scenario {
    name: "inventory_full_curios",
    default_focal: "Cinder",
    default_ticks: 16,
    setup: setup_full_curios,
    expected_features: &[],
};

// ----------------------------------------------------------------------
// inventory_full_herbs — full of HerbHealingMoss, adjacent ground food.
// ----------------------------------------------------------------------

fn setup_full_herbs(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    set_focal_hungry(world, "Cinder");
    fill_focal_inventory(world, "Cinder", ItemKind::HerbHealingMoss, 5);
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(21, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 21));
    spawn_stores_west(world);
    assert_has_ground_carcass(world);
}

pub static SCENARIO_FULL_HERBS: Scenario = Scenario {
    name: "inventory_full_herbs",
    default_focal: "Cinder",
    default_ticks: 16,
    setup: setup_full_herbs,
    expected_features: &[],
};

// ----------------------------------------------------------------------
// inventory_empty_pickup_unchanged — empty cat, adjacent food. Substrate
// path of PickUp fires; no DropItem prefix.
// ----------------------------------------------------------------------

fn setup_empty_pickup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    set_focal_hungry(world, "Cinder");
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(21, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 21));
    spawn_stores_west(world);
    assert_has_ground_carcass(world);
}

pub static SCENARIO_EMPTY_PICKUP: Scenario = Scenario {
    name: "inventory_empty_pickup_unchanged",
    default_focal: "Cinder",
    default_ticks: 16,
    setup: setup_empty_pickup,
    expected_features: &[],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// Cat with 5/5 ShinyPebble + adjacent food: A* composes
    /// `[DropItem, PickUpItemFromGround]` via the plan-path branch.
    /// The full cat must drop a pebble before picking up food, so
    /// the final inventory must include at least one food slot, AND
    /// the dropped pebble must appear as a new ground item.
    #[test]
    fn full_curios_inventory_drops_then_picks_up() {
        let report = run(&SCENARIO_FULL_CURIOS, None, Some(600), 42);
        let counts = report.winner_counts();
        let pickup_wins = counts.get("PickUp").copied().unwrap_or(0);
        assert!(
            pickup_wins >= 1,
            "PickingUp must elect at L3 at least once when food is on ground; got {counts:?}",
        );
        let item_dropped = report
            .feature_counts
            .get("ItemDropped")
            .copied()
            .unwrap_or(0);
        let item_retrieved = report
            .feature_counts
            .get("ItemRetrieved")
            .copied()
            .unwrap_or(0);
        assert!(
            item_dropped >= 1,
            "DropItem must have run (planner composed [DropItem, PickUp] via the plan-path); \
             feature_counts={:?}",
            report.feature_counts,
        );
        assert!(
            item_retrieved >= 1,
            "PickUp resolver must have succeeded after DropItem freed the slot; \
             feature_counts={:?}",
            report.feature_counts,
        );
    }

    /// Same shape with herbs. Pre-231 `resolve_drop_item` filtered to
    /// `ItemSlot::Item` and skipped herbs entirely; herb-clogged cats
    /// were permanently stuck. Post-231 `ItemSlot` collapse + variant-
    /// agnostic slot pick + `drop_priority`'s herb base (0.5) make the
    /// herb droppable when nothing else competes.
    #[test]
    fn full_herbs_inventory_drops_then_picks_up() {
        let report = run(&SCENARIO_FULL_HERBS, None, Some(600), 42);
        let counts = report.winner_counts();
        let pickup_wins = counts.get("PickUp").copied().unwrap_or(0);
        assert!(
            pickup_wins >= 1,
            "PickingUp must elect at L3 at least once when food is on ground; got {counts:?}",
        );
        // Pre-231 `resolve_drop_item` filtered to `ItemSlot::Item`
        // and skipped herb slots — DropItem would Fail on a herb-only
        // inventory. Post-231 the ItemSlot collapse + variant-agnostic
        // resolver picks any slot; ItemDropped fires.
        let item_dropped = report
            .feature_counts
            .get("ItemDropped")
            .copied()
            .unwrap_or(0);
        let item_retrieved = report
            .feature_counts
            .get("ItemRetrieved")
            .copied()
            .unwrap_or(0);
        assert!(
            item_dropped >= 1,
            "DropItem must have run on a herb-clogged cat (validates ItemSlot collapse + \
             variant-agnostic resolver); feature_counts={:?}",
            report.feature_counts,
        );
        assert!(
            item_retrieved >= 1,
            "PickUp resolver must have succeeded after DropItem freed the herb slot; \
             feature_counts={:?}",
            report.feature_counts,
        );
    }

    /// Empty-inventory baseline: substrate-path of PickUp fires
    /// directly (cost 1) without a DropItem prefix (which would cost
    /// 2). A* picks the cheap path — no `ItemDropped` feature fires.
    #[test]
    fn empty_inventory_takes_substrate_path() {
        let report = run(&SCENARIO_EMPTY_PICKUP, None, Some(600), 42);
        let item_dropped = report
            .feature_counts
            .get("ItemDropped")
            .copied()
            .unwrap_or(0);
        let item_retrieved = report
            .feature_counts
            .get("ItemRetrieved")
            .copied()
            .unwrap_or(0);
        assert_eq!(
            item_dropped, 0,
            "DropItem must NOT fire when cat already has free slots — substrate path is cheaper; \
             feature_counts={:?}",
            report.feature_counts,
        );
        assert!(
            item_retrieved >= 1,
            "PickUp should still succeed on an empty cat with adjacent food; feature_counts={:?}",
            report.feature_counts,
        );
    }
}
