//! Hunt-then-deposit pipeline scenario — ticket 184.
//!
//! Tests the canonical Hunt→DepositPrey chain end-to-end: a hungry,
//! skilled hunter with an empty inventory and a Stores building
//! within walking range should kill prey, advance through
//! `TravelTo(Stores)`, and run `DepositPrey` such that
//! `FoodStores.current` increases.
//!
//! Why this scenario exists: ticket 184 surfaced a stockpile-stays-at-zero
//! pattern in a colony soak (post-181 weights). Aggregate footer data
//! showed 362 cats Advancing past EngagePrey but peak stockpile of 9.
//! The arithmetic gap between "Hunt-Advances" and "stockpile" is the
//! exact thing this scenario isolates: with one cat, one Stores, and a
//! handful of prey, does the pipeline land food in Stores at all? If it
//! does (final stockpile ≥ 1), the soak's stockpile collapse is a
//! throughput / equilibrium issue (downstream of L3 bandwidth shift). If
//! it doesn't, there's a structural defect in the kill→deposit chain
//! independent of L3 weights.
//!
//! Default expectation: at least one prey is killed and deposited within
//! 200 ticks. `FoodStores.current` ≥ 1 at end-of-run.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "hunt_deposit_chain",
    default_focal: "Stoat",
    default_ticks: 200,
    setup,
};

/// Sister scenario: same world as `hunt_deposit_chain` but the
/// focal cat starts with the `Injured` marker. Documents the
/// ticket 184 fix (CanHunt no longer gates on `Injured`): a
/// hungry injured cat can still elect Hunt, kill prey, and
/// land food in Stores. Pre-184 this would have been impossible
/// — the eligibility filter would have rejected Hunt every
/// tick, leaving the cat to pick Patrol / Forage / Wander
/// instead.
pub static SCENARIO_INJURED: Scenario = Scenario {
    name: "hunt_deposit_chain_injured",
    default_focal: "Stoat",
    default_ticks: 200,
    setup: setup_injured,
};

fn setup_injured(world: &mut World, seed: u64) {
    setup(world, seed);
    // Stamp the Injured marker on the focal cat. Mirrors what
    // `update_injury_marker` would do in production once the
    // cat's health drops below threshold.
    use crate::components::identity::Name;
    use crate::components::markers::Injured;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let stoat = q
        .iter(world)
        .find(|(_, n)| n.0 == "Stoat")
        .map(|(e, _)| e);
    if let Some(e) = stoat {
        world.entity_mut(e).insert(Injured);
    }
}

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Forest tile near the cat so `update_capability_markers` keeps
    // `CanHunt` asserted across replans (the marker is gated on
    // forest-nearby; default scenario terrain is all Grass, which
    // strips CanHunt the first time the per-tick author runs and
    // makes Hunt's eligibility filter reject every subsequent score).
    {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = world.resource_mut::<TileMap>();
        if map.in_bounds(21, 20) {
            map.get_mut(21, 20).terrain = Terrain::LightForest;
        }
    }

    // Stores building at (18, 20) — two tiles west of the cat, on the
    // opposite side from the prey, so the kill→travel→deposit chain is
    // a real spatial trip rather than colocated.
    use crate::components::building::{StoredItems, Structure, StructureType};
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        Position::new(18, 20),
    ));

    // FoodStores resource starts at 0 — `sync_food_stores` derives
    // capacity from the live Stores building's `effective_capacity_with_items`,
    // so the scenario doesn't need to seed `FoodStores` directly.

    let _stoat = spawn_cat(
        world,
        CatPreset::adult("Stoat", Position::new(20, 20))
            .with_personality(|p| {
                p.boldness = 0.85;
                p.diligence = 0.7;
                p.patience = 0.7;
            })
            .with_needs(|n| {
                // Hunger 0.55 — just above the
                // `production_self_eat_threshold` (0.5). Below
                // threshold the kill resolver eats the catch in
                // place instead of pushing it to inventory, which
                // short-circuits the deposit chain we're testing
                // (see `goap.rs:5388`). Above threshold the cat
                // pushes prey to inventory and proceeds to
                // TravelTo(Stores) → DepositPrey, which is what
                // ticket 184 needs to verify.
                n.hunger = 0.55;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Five prey clustered east of the cat. With cat capacity 5 and
    // five prey within hunt range, a successful pipeline kills,
    // fills inventory, advances to Stores, and deposits — the exact
    // chain ticket 184 is testing.
    spawn_prey_at(world, Position::new(24, 20), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(25, 21), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(25, 19), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(26, 20), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(24, 22), PreyKind::Mouse);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// Regression assertion for ticket 184: an empty-inventory hunter
    /// near prey and Stores should land at least one food item in
    /// Stores within 200 ticks. If this fails, the kill→deposit chain
    /// is structurally broken (independent of any L3-weight
    /// configuration).
    #[test]
    fn pipeline_lands_food_in_stores() {
        let report = run(&SCENARIO, None, Some(200), 42);
        assert!(
            report.final_food_capacity > 0.0,
            "Stores building should contribute capacity (>0); got {}",
            report.final_food_capacity
        );
        assert!(
            report.final_food_current >= 1.0,
            "kill→deposit chain should land at least 1 food in Stores within 200 ticks; got {}/{}",
            report.final_food_current,
            report.final_food_capacity
        );
    }

    /// Ticket 184 fix lock: an injured cat can still elect Hunt and
    /// land food in Stores. Pre-184 the `CanHunt` eligibility filter
    /// rejected injured cats, so the kill→deposit chain was
    /// unreachable for any cat with the `Injured` marker. Post-184
    /// injury dampens via L2 scoring (skill + health interoception)
    /// without disabling eligibility — a one-eyed mangy cat still
    /// hunts rats. If this regresses, the over-gating has come back.
    #[test]
    fn injured_cat_still_lands_food() {
        let report = run(&SCENARIO_INJURED, None, Some(200), 42);
        assert!(
            report.final_food_capacity > 0.0,
            "Stores capacity should be > 0; got {}",
            report.final_food_capacity
        );
        assert!(
            report.final_food_current >= 1.0,
            "injured cat should still land >= 1 food; got {}/{}. \
             pre-184 regression: injury removed CanHunt → Patrol won \
             via Blind commitment + long plans → 0 food deposited.",
            report.final_food_current,
            report.final_food_capacity
        );
    }
}
