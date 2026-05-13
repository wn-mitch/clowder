//! 308 — ColonyReservesBelief substrate first-light.
//!
//! Verifies the new belief substrate fires end-to-end at the scenario
//! scale:
//!
//! 1. Aggregator: `sync_colony_reserves` counts every cat's `Inventory`
//!    plus all Stores buildings into the ground-truth `ColonyReserves`
//!    resource.
//! 2. Witness emission: `gossip_inventory_observations` broadcasts each
//!    cat's inventory on its stagger tick; nearby cats integrate the
//!    observation via `belief_integrator`'s Pass A.
//! 3. Marker authoring: when the focal cat's `ColonyReservesBelief`
//!    `estimated_count <= low_ward_reserve_threshold` (default 2),
//!    `update_low_ward_reserve_markers` inserts `HasLowWardReserve`.
//!
//! Setup: priestess `Sage` at (20, 20) carrying 1 thornbriar, with three
//! witness cats clustered within `WITNESS_RANGE`. A corrupted hot-spot
//! east pulls Sage into `HerbcraftSetWard`. After she consumes her
//! thornbriar, the colony reserve drops to zero — well below the marker
//! threshold of 2. The scenario asserts `HasLowWardReserve` appears on
//! at least one cat within the tick budget.
//!
//! The marker has no DSE consumer in 308 (ticket 309 lands that). The
//! scenario's job is to prove the substrate fires; it doesn't depend on
//! downstream behavior change.

use bevy_ecs::world::World;

use crate::components::magic::HerbKind;
use crate::components::physical::Position;

use super::env::{give_herbs, init_scenario_world, mark_tile_corrupted, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "colony_reserves_belief",
    default_focal: "Sage",
    default_ticks: 120,
    setup,
    // L2/L3 election triage scenario — substrate-firing is verified by
    // the test below rather than via Feature canaries (the belief is
    // dormant in 308; no Feature emits).
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Corruption gradient east of Sage to pull Herbalism / SetWard.
    mark_tile_corrupted(world, Position::new(24, 20), 0.7);
    for dx in -2..=2 {
        for dy in -2..=2 {
            mark_tile_corrupted(world, Position::new(24 + dx, 20 + dy), 0.4);
        }
    }

    let sage = spawn_cat(
        world,
        CatPreset::adult("Sage", Position::new(20, 20))
            .with_personality(|p| {
                p.spirituality = 0.85;
                p.diligence = 0.7;
                p.compassion = 0.7;
            })
            .with_magic_affinity(0.6)
            .with_marker(MarkerKind::Adult),
    );
    // Sage carries exactly one thornbriar — the colony's entire reserve.
    // She'll consume it on the first SetWard attempt, dropping the
    // colony pool to zero.
    give_herbs(world, sage, HerbKind::Thornbriar, 1);

    // Three witnesses clustered within `WITNESS_RANGE = 10` of Sage so
    // they integrate her inventory broadcasts.
    for (i, name) in ["Birch", "Lark", "Bramble"].iter().enumerate() {
        spawn_cat(
            world,
            CatPreset::adult(*name, Position::new(21 + i as i32, 22))
                .with_marker(MarkerKind::Adult),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::markers::HasLowWardReserve;
    use crate::scenarios::runner::build_scenario_app;

    /// 308 substrate first-light: the marker chain fires end-to-end
    /// once Sage's thornbriar is consumed and her stagger-tick
    /// `InventoryObserved` broadcasts the now-empty inventory to
    /// witnesses.
    #[test]
    fn has_low_ward_reserve_fires_within_tick_budget() {
        let mut app = build_scenario_app(42, &SCENARIO, "Sage");
        // First update runs Startup (scenario setup).
        app.update();
        for _ in 0..SCENARIO.default_ticks {
            app.update();
        }

        let world = app.world_mut();
        let mut q = world.query::<bevy_ecs::entity::Entity>();
        let count = q
            .iter(world)
            .filter(|e| world.entity(*e).contains::<HasLowWardReserve>())
            .count();

        assert!(
            count > 0,
            "expected at least one cat to carry HasLowWardReserve \
             after Sage consumes the colony's only thornbriar within \
             {} ticks (count = {count})",
            SCENARIO.default_ticks,
        );
    }
}
