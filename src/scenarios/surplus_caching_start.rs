//! Ethological colony-start — surplus-caching first-light.
//!
//! Proves the surplus-caching substrate fires end-to-end at scenario
//! scale, with the dormant levers activated (the landing default is
//! byte-identical; this fixture flips them on to exercise the mechanism):
//!
//! 1. **C2 — surplus perception.** Founders spawn amid an ungathered
//!    scatter of OnGround food and no Stores. `update_ground_surplus_map`
//!    stamps the scatter; `integrate_beliefs` Pass B authors each founder's
//!    per-location `surplus_food` belief on its stagger tick. Asserted:
//!    at least one founder's `LocationBeliefs` carries a nonzero
//!    `surplus_food` facet.
//!
//! 2. **C3 — build a larder because there is none.** With no coordinator
//!    elected yet and no Stores building, `assess_colony_needs`'s
//!    colony-self branch emits a `Build{ blueprint: Stores }` directive
//!    (gated on `colony_self_no_store_priority > 0.0`). The directive
//!    dispatches, a founder works it, and a Stores structure/site appears.
//!    Asserted: a `Structure` of kind `Stores` (site or finished) exists
//!    within the tick budget — the colony started building its larder
//!    because it had food to store and nowhere to put it.
//!
//! The landing default keeps all four levers at 0.0 (seed-42-neutral); a
//! first-light activation soak lifts them together. This fixture is the
//! deterministic mechanism proof that guards against silent breakage.

use bevy_ecs::world::World;

use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position::new(20, 20);

pub static SCENARIO: Scenario = Scenario {
    name: "surplus_caching_start",
    default_focal: "Ash",
    default_ticks: 400,
    setup,
    // Belief + directive substrate; verified by the tests below rather
    // than Feature canaries (SurplusFoodBeliefFormed ships expected=false
    // until first-light activation observes it in a real soak).
    expected_features: &[],
};

/// Scatter ungathered food around the colony center — the founding
/// windfall a cat could gather and cache. Mirrors the founding scatter in
/// `world_gen/colony.rs` (OnGround food Items with a Position).
fn scatter_ground_food(world: &mut World) {
    let kinds = [
        ItemKind::RawMouse,
        ItemKind::Berries,
        ItemKind::Nuts,
        ItemKind::RawFish,
    ];
    let offsets = [
        (2, 0),
        (0, 2),
        (-2, 0),
        (0, -2),
        (3, 1),
        (-3, -1),
        (1, 3),
        (-1, -3),
    ];
    for (i, (dx, dy)) in offsets.iter().enumerate() {
        let kind = kinds[i % kinds.len()];
        let pos = Position::new(COLONY_CENTER.x() + dx, COLONY_CENTER.y() + dy);
        world.spawn((Item::new(kind, 1.0, ItemLocation::OnGround), pos));
    }
}

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Activate the dormant levers so the fixture exercises the mechanism.
    {
        let mut constants = world.resource_mut::<crate::resources::SimConstants>();
        // C3 — colony-self new-Stores trigger (off at land).
        constants.coordination.colony_self_no_store_priority = 0.8;
        // C1 — surplus-caching drives (off at land). Not strictly required
        // for the assertions below, but activated so the fixture also
        // exercises the Forage/PickUp/Build surplus axes without a store
        // yet (they stay inert until a Stores is reachable).
        constants.scoring.forage_surplus_cache_weight = 0.3;
        constants.scoring.pickup_surplus_weight = 1.0;
        constants.scoring.build_surplus_food_weight = 0.2;
    }

    // Eight well-fed founders clustered at the colony center. Well-fed so
    // personal hunger doesn't dominate — the colony-provisioning story, not
    // a starvation response.
    let names = [
        "Ash", "Birch", "Cedar", "Dune", "Ember", "Fern", "Gale", "Holly",
    ];
    for (i, name) in names.iter().enumerate() {
        let pos = Position::new(
            COLONY_CENTER.x() + (i as i32 % 3),
            COLONY_CENTER.y() + (i as i32 / 3),
        );
        let cat = spawn_cat(
            world,
            CatPreset::adult(*name, pos).with_marker(MarkerKind::Adult),
        );
        // Sated (hunger 1.0 = full in this model).
        if let Some(mut needs) = world
            .entity_mut(cat)
            .get_mut::<crate::components::physical::Needs>()
        {
            needs.hunger = 0.9;
        }
    }

    scatter_ground_food(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::building::{Structure, StructureType};
    use crate::scenarios::runner::build_scenario_app;

    #[test]
    fn surplus_belief_forms_on_a_founder() {
        use crate::resources::system_activation::{Feature, SystemActivation};

        let mut app = build_scenario_app(42, &SCENARIO, "Ash");
        app.update(); // Startup (scenario setup).
        for _ in 0..SCENARIO.default_ticks {
            app.update();
        }

        // Assert on the monotonic Feature counter rather than the live
        // belief state: with the caching levers active, founders gather the
        // scatter and the slow-decay `surplus_food` facet can fade below
        // strength by the end of the budget. `SurplusFoodBeliefFormed` records
        // every lift, so it's decay-immune — and this doubles as the canary
        // that must fire ≥1× before promoting `expected_to_fire` to true.
        let world = app.world_mut();
        let fired = world
            .resource::<SystemActivation>()
            .counts
            .get(&Feature::SurplusFoodBeliefFormed)
            .copied()
            .unwrap_or(0);
        assert!(
            fired > 0,
            "expected the SurplusFoodBeliefFormed canary to fire ≥1× as founders \
             perceive the ungathered scatter within {} ticks; fired {fired}×",
            SCENARIO.default_ticks,
        );
    }

    #[test]
    fn colony_builds_a_stores_when_it_has_none() {
        let mut app = build_scenario_app(42, &SCENARIO, "Ash");
        app.update(); // Startup (scenario setup).
                      // Sanity: no Stores at the start.
        {
            let world = app.world_mut();
            let mut q = world.query::<&Structure>();
            let stores_at_start = q
                .iter(world)
                .filter(|s| s.kind == StructureType::Stores)
                .count();
            assert_eq!(stores_at_start, 0, "fixture must start with no Stores");
        }
        for _ in 0..SCENARIO.default_ticks {
            app.update();
        }

        let world = app.world_mut();
        let mut q = world.query::<&Structure>();
        let stores_count = q
            .iter(world)
            .filter(|s| s.kind == StructureType::Stores)
            .count();
        assert!(
            stores_count > 0,
            "expected the colony-self no-store trigger to place a Stores \
             (site or finished) within {} ticks; found {stores_count}",
            SCENARIO.default_ticks,
        );
    }
}
