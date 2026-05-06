//! Ticket 178 — election-side scenarios for the lifted disposal DSEs.
//!
//! Sister to `disposal_dispatch.rs` (177's hand-stamped-plan dispatch
//! tests). These scenarios run the full L2/L3 pipeline: a cat with a
//! food-stuffed inventory and the right colony substrate (Midden
//! present / `ColonyStoresChronicallyFull` set) should *organically
//! elect* the matching disposal disposition rather than picking
//! Hunt/Forage/Patrol.
//!
//! Each scenario isolates one election surface:
//!
//! - **`disposal_election_trashing`** — Midden present →
//!   Trashing wins L3.
//! - **`disposal_election_discarding`** — no Midden,
//!   `ColonyStoresChronicallyFull` latched → Discarding wins L3.
//! - **`disposal_election_idle`** — empty inventory, no chronic
//!   marker → neither disposal DSE wins (the curves saturate to
//!   near-zero on `inventory_excess = 0` and the eligibility filters
//!   reject without supporting substrate).
//! - **`disposal_election_discarding_blocked_without_marker`** —
//!   full inventory but `ColonyStoresChronicallyFull` absent → the
//!   eligibility filter rejects Discarding; with no Midden either,
//!   the cat falls back to Hunt/Forage/Patrol.

use bevy_ecs::world::World;

use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::items::ItemKind;
use crate::components::physical::Position;
use crate::resources::stores_pressure::StoresPressureTracker;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position { x: 20, y: 20 };

/// Stuff `count` `RawMouse` items into the focal cat's inventory.
/// Goes through `Inventory::add_item` (preserves the production
/// MAX_SLOTS gate) so the inventory is genuinely "full" and
/// `food_count() / MAX_SLOTS == 1.0` — exactly the input
/// `inventory_excess` reads.
fn fill_focal_inventory_with_food(world: &mut World, focal_name: &str, count: usize) {
    use crate::components::identity::Name;
    use crate::components::magic::Inventory;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist before filling inventory");
    let mut em = world.entity_mut(entity);
    let mut inv = em.get_mut::<Inventory>().expect("focal has Inventory");
    for _ in 0..count {
        inv.add_item(ItemKind::RawMouse);
    }
}

/// Latch `ColonyStoresChronicallyFull` for the duration of the run.
/// `update_colony_building_markers` re-asserts the marker each tick
/// from `tracker.latched_chronic`, so flipping the latch and parking
/// `last_window_tick` at the current tick keeps the marker present
/// without needing to simulate `Feature::DepositRejected` accumulation.
/// Also pre-inserts the marker on the colony singleton so tick 1's
/// scoring pass sees it (the author system runs after
/// `evaluate_and_plan` on the first tick).
fn latch_chronic_full(world: &mut World) {
    let tick = world.resource::<crate::resources::TimeState>().tick;
    {
        let mut tracker = world.resource_mut::<StoresPressureTracker>();
        tracker.latched_chronic = true;
        tracker.last_window_tick = tick;
    }
    let colony = world
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<crate::components::markers::ColonyState>>()
        .iter(world)
        .next()
        .expect("ColonyState singleton must exist");
    world
        .entity_mut(colony)
        .insert(crate::components::markers::ColonyStoresChronicallyFull);
}

/// Pre-insert `HasMidden` on the colony singleton. Same race guard
/// as `latch_chronic_full`: the building-marker author system runs
/// after `evaluate_and_plan` on tick 1, so without this pre-insert
/// Trashing's eligibility filter rejects on the first scoring pass.
fn assert_has_midden(world: &mut World) {
    let colony = world
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<crate::components::markers::ColonyState>>()
        .iter(world)
        .next()
        .expect("ColonyState singleton must exist");
    world
        .entity_mut(colony)
        .insert(crate::components::markers::HasMidden);
}

// ----------------------------------------------------------------------
// disposal_election_trashing — Midden present, full inventory,
// ColonyStoresChronicallyFull latched. Both disposal DSEs gate on the
// chronic-full marker (otherwise cats trash food the Stores could
// still accept); HasMidden differentiates Trashing from Discarding.
// Trashing wins because both DSEs score the same `inventory_excess`
// shape and Trashing's eligibility passes (Midden present); Discarding
// would also be eligible, so the softmax may give either; the test
// asserts Trashing wins at least once.
// ----------------------------------------------------------------------

fn setup_trashing(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    world.spawn((
        Structure::new(StructureType::Midden),
        StoredItems::default(),
        Position::new(21, 20),
    ));
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    fill_focal_inventory_with_food(world, "Cinder", 5);
    assert_has_midden(world);
    latch_chronic_full(world);
}

pub static SCENARIO_TRASHING: Scenario = Scenario {
    name: "disposal_election_trashing",
    default_focal: "Cinder",
    default_ticks: 5,
    setup: setup_trashing,
};

// ----------------------------------------------------------------------
// disposal_election_discarding — no Midden, ColonyStoresChronicallyFull
// latched, full inventory. Trashing eligibility-filtered out; Discarding
// fires.
// ----------------------------------------------------------------------

fn setup_discarding(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    fill_focal_inventory_with_food(world, "Cinder", 5);
    latch_chronic_full(world);
}

pub static SCENARIO_DISCARDING: Scenario = Scenario {
    name: "disposal_election_discarding",
    default_focal: "Cinder",
    default_ticks: 5,
    setup: setup_discarding,
};

// ----------------------------------------------------------------------
// disposal_election_idle — empty inventory, no chronic marker, no Midden.
// Neither DSE wins (curves saturate to near-zero AND eligibility filters
// reject).
// ----------------------------------------------------------------------

fn setup_idle(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
}

pub static SCENARIO_IDLE: Scenario = Scenario {
    name: "disposal_election_idle",
    default_focal: "Cinder",
    default_ticks: 5,
    setup: setup_idle,
};

// ----------------------------------------------------------------------
// disposal_election_discarding_blocked_without_marker — full inventory,
// ColonyStoresChronicallyFull NOT latched, no Midden. Both eligibility
// filters reject; the cat falls back to a non-disposal disposition
// (Hunt/Forage/Patrol/Wander/...).
// ----------------------------------------------------------------------

fn setup_discarding_blocked(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    fill_focal_inventory_with_food(world, "Cinder", 5);
}

pub static SCENARIO_DISCARDING_BLOCKED: Scenario = Scenario {
    name: "disposal_election_discarding_blocked_without_marker",
    default_focal: "Cinder",
    default_ticks: 5,
    setup: setup_discarding_blocked,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// With a Midden present, Trashing wins L3 election within the
    /// 5-tick budget. Asserts winner-counts so a single skipped tick
    /// (e.g., the FocalTraceTarget warm-up tick that emits an empty
    /// row) doesn't cause a false negative.
    #[test]
    fn trashing_wins_with_midden() {
        let report = run(&SCENARIO_TRASHING, None, Some(5), 42);
        let counts = report.winner_counts();
        let trash_wins = counts.get("Trash").copied().unwrap_or(0);
        assert!(
            trash_wins >= 1,
            "Trashing (Action::Trash) should win L3 at least once with a Midden present and full inventory; \
             got {counts:?}",
        );
    }

    /// With ColonyStoresChronicallyFull latched and no Midden,
    /// Discarding wins L3.
    #[test]
    fn discarding_wins_with_chronic_full_marker() {
        let report = run(&SCENARIO_DISCARDING, None, Some(5), 42);
        let counts = report.winner_counts();
        let discard_wins = counts.get("Drop").copied().unwrap_or(0);
        assert!(
            discard_wins >= 1,
            "Discarding (Action::Drop) should win L3 at least once with ColonyStoresChronicallyFull latched and \
             a full food inventory; got {counts:?}",
        );
    }

    /// Empty-inventory baseline: neither disposal DSE wins.
    /// `inventory_excess = 0.0` puts the Logistic(8, 0.5) curve at
    /// `~0.018`, well below any other DSE that scores on actual need.
    #[test]
    fn neither_disposal_wins_when_idle() {
        let report = run(&SCENARIO_IDLE, None, Some(5), 42);
        let counts = report.winner_counts();
        let discard_wins = counts.get("Drop").copied().unwrap_or(0);
        let trash_wins = counts.get("Trash").copied().unwrap_or(0);
        assert_eq!(
            discard_wins, 0,
            "Discarding (Drop) must not win on an empty-inventory cat with no chronic marker; \
             got {counts:?}",
        );
        assert_eq!(
            trash_wins, 0,
            "Trashing (Trash) must not win on an empty-inventory cat with no Midden; got {counts:?}",
        );
    }

    /// Eligibility-filter regression test: full inventory but no
    /// supporting substrate (no Midden, no ColonyStoresChronicallyFull).
    /// Both filters reject; the cat picks a non-disposal disposition.
    /// This is the safety check that prevents Discarding/Trashing from
    /// ever firing on a healthy colony with available stores.
    #[test]
    fn neither_disposal_wins_without_supporting_substrate() {
        let report = run(&SCENARIO_DISCARDING_BLOCKED, None, Some(5), 42);
        let counts = report.winner_counts();
        let discard_wins = counts.get("Drop").copied().unwrap_or(0);
        let trash_wins = counts.get("Trash").copied().unwrap_or(0);
        assert_eq!(
            discard_wins, 0,
            "Discarding (Drop) eligibility filter must reject without ColonyStoresChronicallyFull; \
             got {counts:?}",
        );
        assert_eq!(
            trash_wins, 0,
            "Trashing (Trash) eligibility filter must reject without HasMidden; got {counts:?}",
        );
    }
}
