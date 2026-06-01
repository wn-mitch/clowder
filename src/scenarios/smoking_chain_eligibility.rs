//! Smoking-chain eligibility microexperiment — ticket 443.
//!
//! Three sister fixtures isolate `SmokeMeatDse` eligibility under the
//! 443 composite-marker fix (`HasSmokeableAccessible`). Mirrors the
//! drying-chain eligibility suite (`drying_chain_eligibility.rs`).
//!
//! Pre-443 the DSE required `HasSmokeableInInventory` only — cats
//! deposit raw meat at Stores on hunt-return so the per-cat inventory
//! marker is false at scoring time, silently filtering the DSE. The
//! fix widens eligibility via `HasSmokeableAccessible`:
//!   inventory has both meat+fuel  OR  (free slot AND `HasSmokeableInStores`)
//!
//! Fixtures:
//! - **hot_inventory** — cat carries raw meat + fuel; `smoke_meat` must
//!   be eligible (left disjunct).
//! - **stores_has_smokeable** — empty inventory, Stores has raw meat +
//!   fuel; `smoke_meat` must be eligible via composite (right disjunct).
//! - **empty_stores** — nothing smokeable anywhere; `smoke_meat` must
//!   be ineligible (negative control).

use bevy_ecs::world::World;

use crate::components::building::{SmokingRackState, StoredItems, Structure, StructureType};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::Inventory;
use crate::components::physical::{Needs, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position::new(20, 20);
const RACK_POS: Position = Position::new(21, 20);
const STORES_POS: Position = Position::new(19, 20);

const DEFAULT_TICKS: u32 = 10;

pub static SCENARIO_HOT_INVENTORY: Scenario = Scenario {
    name: "smoking_chain_hot_inventory",
    default_focal: "Cinder",
    default_ticks: DEFAULT_TICKS,
    setup: setup_hot_inventory,
    expected_features: &[],
};

pub static SCENARIO_STORES_HAS_SMOKEABLE: Scenario = Scenario {
    name: "smoking_chain_stores_has_smokeable",
    default_focal: "Cinder",
    default_ticks: DEFAULT_TICKS,
    setup: setup_stores_has_smokeable,
    expected_features: &[],
};

pub static SCENARIO_EMPTY_STORES: Scenario = Scenario {
    name: "smoking_chain_empty_stores",
    default_focal: "Cinder",
    default_ticks: DEFAULT_TICKS,
    setup: setup_empty_stores,
    expected_features: &[],
};

// -----------------------------------------------------------------------
// Fixture 1: hot inventory
// -----------------------------------------------------------------------
//
// Cat carries RawMouse + Wood. Functional SmokingRack one tile away.
// Left disjunct of `HasSmokeableAccessible` should hold.
fn setup_hot_inventory(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_smoking_rack(world, RACK_POS);
    spawn_empty_stores(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_smokemeat_baseline_needs),
    );
    fill_focal_inventory_with_smokeable(world, "Cinder");
}

// -----------------------------------------------------------------------
// Fixture 2: empty inventory + stores has smokeable
// -----------------------------------------------------------------------
//
// Cat has empty inventory. Stores holds RawMouse + Wood. Right disjunct
// of `HasSmokeableAccessible` (`HasFreeSlot && HasSmokeableInStores`)
// should make `smoke_meat` eligible. This is the Commit 10 path.
fn setup_stores_has_smokeable(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_smoking_rack(world, RACK_POS);
    spawn_stores_with_meat_and_fuel(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_smokemeat_baseline_needs),
    );
}

// -----------------------------------------------------------------------
// Fixture 3: empty inventory + empty stores (negative control)
// -----------------------------------------------------------------------
//
// Neither disjunct holds. `smoke_meat` must be ineligibility-filtered.
fn setup_empty_stores(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_smoking_rack(world, RACK_POS);
    spawn_empty_stores(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_smokemeat_baseline_needs),
    );
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn spawn_smoking_rack(world: &mut World, pos: Position) {
    world.spawn((
        Structure::new(StructureType::SmokingRack),
        SmokingRackState::default(),
        pos,
    ));
}

fn spawn_empty_stores(world: &mut World, pos: Position) {
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        pos,
    ));
}

fn spawn_stores_with_meat_and_fuel(world: &mut World, pos: Position) {
    let stores = world
        .spawn((
            Structure::new(StructureType::Stores),
            StoredItems::default(),
            pos,
        ))
        .id();
    // One raw-meat item.
    let meat = world
        .spawn(Item::new(
            ItemKind::RawMouse,
            1.0,
            ItemLocation::StoredIn(stores),
        ))
        .id();
    // One fuel item.
    let fuel = world
        .spawn(Item::new(
            ItemKind::Wood,
            1.0,
            ItemLocation::StoredIn(stores),
        ))
        .id();
    let mut em = world.entity_mut(stores);
    let mut stored = em
        .get_mut::<StoredItems>()
        .expect("Stores must have StoredItems");
    stored.items.push(meat);
    stored.items.push(fuel);
}

fn fill_focal_inventory_with_smokeable(world: &mut World, focal_name: &str) {
    use crate::components::identity::Name;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist before fill_focal_inventory_with_smokeable");
    let mut em = world.entity_mut(entity);
    let mut inv = em.get_mut::<Inventory>().expect("focal has Inventory");
    inv.add_item(ItemKind::RawMouse);
    inv.add_item(ItemKind::Wood);
}

fn set_smokemeat_baseline_needs(n: &mut Needs) {
    n.hunger = 0.7;
    n.energy = 0.9;
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    fn first_smoke_meat_row(
        report: &crate::scenarios::runner::ScenarioReport,
    ) -> Option<(bool, f32)> {
        for t in &report.ticks {
            if let Some(row) = t.l2.iter().find(|r| r.dse == "smoke_meat") {
                return Some((row.eligible, row.final_score));
            }
        }
        None
    }

    #[test]
    fn hot_inventory_makes_smoke_meat_eligible() {
        let report = run(&SCENARIO_HOT_INVENTORY, None, Some(DEFAULT_TICKS), 42);
        let (eligible, score) = first_smoke_meat_row(&report).unwrap_or_else(|| {
            panic!(
                "smoke_meat never surfaced in any L2 table — DSE not scoring at all. \
                 Ticks captured: {}",
                report.ticks.len()
            )
        });
        assert!(
            eligible,
            "smoke_meat should be eligible when cat has raw meat + fuel in inventory \
             (left disjunct of HasSmokeableAccessible). Got eligible=false, \
             final_score={score}."
        );
    }

    #[test]
    fn stores_has_smokeable_makes_smoke_meat_eligible_via_composite() {
        let report = run(
            &SCENARIO_STORES_HAS_SMOKEABLE,
            None,
            Some(DEFAULT_TICKS),
            42,
        );
        let (eligible, score) = first_smoke_meat_row(&report).unwrap_or_else(|| {
            panic!(
                "smoke_meat never surfaced in any L2 table. \
                 Ticks captured: {}",
                report.ticks.len()
            )
        });
        assert!(
            eligible,
            "smoke_meat should be eligible via the Commit 10 composite-marker path \
             (empty inv + colony has meat+fuel in Stores). Got eligible=false, \
             final_score={score}. Inspect HasSmokeableInStores (buildings.rs) and \
             HasSmokeableAccessible composite at goap.rs."
        );
    }

    #[test]
    fn empty_stores_filters_smoke_meat() {
        let report = run(&SCENARIO_EMPTY_STORES, None, Some(DEFAULT_TICKS), 42);
        match first_smoke_meat_row(&report) {
            None => {}
            Some((eligible, score)) => assert!(
                !eligible,
                "smoke_meat should be filtered when nothing is smokeable anywhere. \
                 Got eligible=true, final_score={score}. HasSmokeableAccessible is leaky."
            ),
        }
    }
}
