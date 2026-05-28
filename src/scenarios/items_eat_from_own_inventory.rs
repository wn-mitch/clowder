//! Ticket 429 — items-are-real Sink contract for eat-from-own-inventory.
//!
//! Preloads a single adult cat at hunger 0.3 (below the default
//! `eat_from_inventory_threshold` = 0.4) with one `RawMouse` in its
//! pouch, then advances a few ticks. The per-tick autonomic dispatcher
//! at `src/systems/needs.rs::eat_from_inventory` routes through the
//! `resolve_eat_from_own_inventory` Sink, which drains the slot,
//! credits hunger, and fires `Feature::EatFromOwnInventory` via
//! `record_if_witnessed`. The scenario asserts all three.
//!
//! Pre-429 the dispatcher mutated `inventory.take_food()` inline with
//! no Feature emission and no resolver indirection — the substrate
//! contract was satisfied behaviorally but bypassed the named gate.
//! This scenario locks in the contract: any future refactor that
//! breaks the gate (e.g., the dispatcher stops calling the resolver,
//! the resolver stops emitting its witness, the Feature gets renamed
//! without updating the dispatcher) trips a deterministic, ~3-second
//! signal — cheaper than waiting for a 15-min soak's never-fired
//! canary to surface the dead Sink.

use bevy_ecs::world::World;

use crate::components::items::ItemKind;
use crate::components::magic::Inventory;
use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "items_eat_from_own_inventory",
    default_focal: "Pocket",
    // 3 ticks is sufficient — the autonomic dispatcher fires per-tick
    // on every cat with hunger < threshold AND food in inventory, so
    // the resolver runs in the first FixedUpdate tick after Startup.
    default_ticks: 3,
    setup,
    // Asserted in unit tests below; scenario harness's
    // `expected_features` would only run with `just scenario` (which
    // doesn't enroll mod-test assertions). The test path also lets us
    // verify the slot-drain + hunger-credit side effects, which the
    // harness assert can't reach.
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let pocket = spawn_cat(
        world,
        CatPreset::adult("Pocket", Position::new(20, 20))
            .with_marker(MarkerKind::Adult)
            .with_needs(|n| {
                n.hunger = 0.30;
            }),
    );

    let added = world
        .entity_mut(pocket)
        .get_mut::<Inventory>()
        .expect("focal cat must have Inventory")
        .add_item(ItemKind::RawMouse);
    assert!(added, "fixture: focal cat inventory must accept a RawMouse");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::system_activation::{Feature, SystemActivation};
    use crate::scenarios::runner::build_scenario_app;
    use bevy::app::App;
    use bevy_ecs::prelude::Entity;

    fn run_for(ticks: u32) -> App {
        let mut app = build_scenario_app(42, &SCENARIO, SCENARIO.default_focal);
        app.update(); // Startup
        for _ in 0..ticks {
            app.update();
        }
        app
    }

    fn find_cat_by_name(world: &mut bevy_ecs::world::World, name: &str) -> Entity {
        let mut q = world.query::<(Entity, &crate::components::identity::Name)>();
        for (entity, n) in q.iter(world) {
            if n.0.as_str() == name {
                return entity;
            }
        }
        panic!("no cat named {name}");
    }

    /// Drains the slot, credits hunger, fires the canary.
    #[test]
    fn dispatcher_routes_through_sink_and_fires_canary() {
        let mut app = run_for(SCENARIO.default_ticks);
        let world = app.world_mut();
        let pocket = find_cat_by_name(world, "Pocket");

        let inv = world.entity(pocket).get::<Inventory>().expect("inventory");
        assert_eq!(
            inv.pouch.len(),
            0,
            "Sink should drain the food slot; got {} slot(s)",
            inv.pouch.len()
        );

        let needs = world
            .entity(pocket)
            .get::<crate::components::physical::Needs>()
            .expect("needs");
        assert!(
            needs.hunger > 0.30,
            "Sink should credit hunger above the starting 0.30; got {}",
            needs.hunger
        );

        let activation = world.resource::<SystemActivation>();
        let count = activation
            .counts
            .get(&Feature::EatFromOwnInventory)
            .copied()
            .unwrap_or(0);
        assert!(
            count >= 1,
            "Feature::EatFromOwnInventory must fire ≥ 1× when the Sink drains a slot; counts={:?}",
            activation.counts
        );
    }
}
