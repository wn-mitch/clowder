//! Ticket 193 / 191 — PickingUp DSE scenarios. Two siblings sharing
//! the same carcass-spawn layout but differing in the slice of the
//! pipeline they isolate.
//!
//! Both scenarios spawn a focal cat with an empty inventory and three
//! OnGround food `Item` entities (no live prey). The `HasGroundCarcass`
//! colony marker (185 → 193 re-wire to the food-Item surface) makes
//! PickingUp eligible; the inverted-Logistic `colony_food_security`
//! curve (185) scores it high; the `PlannerZone::CarcassPile` (193)
//! resolves to a real position so A* produces a viable plan.
//!
//! Pre-193 baseline: the plan template routed through
//! `PlannerZone::MaterialPile`, which filtered to build materials
//! only — A* found no target, the plan failed with `GoalUnreachable`,
//! and the cat replanned every tick. The seed-42 canonical soak
//! recorded 1367 such failures per 10kt, driving colony collapse.
//!
//! `SCENARIO` (193): hunger 0.2, no Stores, 16-tick budget. Asserts
//! L3 election + resolver execution for the *pickup* leg. Hunger is
//! low so the cat is starving and PickingUp dominates the L3 softmax
//! over Forage; no Stores so the cat can't accidentally deposit.
//!
//! `SCENARIO_TO_STORES` (191): hunger 0.55, Stores building four
//! tiles west, 200-tick budget. Asserts the full scavenge → travel →
//! deposit chain — `FoodStores.current >= 1` at end. Hunger sits
//! above the kill-resolver's `production_self_eat_threshold` (0.5;
//! see `hunt_deposit_chain.rs`) so the cat carries the picked-up
//! carcass to Stores rather than eating in place. `food_fraction`
//! stays pinned at 0 (empty Stores) so `colony_food_security =
//! min(food_fraction, hunger_satisfaction) = 0` and PickingUp still
//! scores ~0.99 — but the longer budget lets a non-starving cat
//! reach PickingUp through the L3 softmax variance.

use bevy_ecs::world::World;

use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::physical::{Needs, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position::new(20, 20);

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

/// Set the focal cat's hunger. `colony_food_security` reads
/// `min(food_fraction, hunger_satisfaction)` so the value chosen
/// here also clamps PickingUp's curve input, but only when
/// `food_fraction` itself isn't already 0 from an empty FoodStores.
fn set_focal_hunger(world: &mut World, focal_name: &str, hunger: f32) {
    use crate::components::identity::Name;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist before set_focal_hunger");
    let mut em = world.entity_mut(entity);
    let mut needs = em.get_mut::<Needs>().expect("focal has Needs");
    needs.hunger = hunger;
}

/// Three RawMouse Items in an L-shape east of the cat at (20, 20).
/// Used by both scenarios so the spatial layout of the pickup target
/// is identical; only the deposit destination differs.
fn spawn_three_carcasses_east(world: &mut World) {
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(22, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 22));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(18, 20));
}

fn setup_picking_up_scavenging(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    // 0.2 — starving. Empty `FoodStores` already pins `food_fraction`
    // to 0, but making hunger small too keeps the composite far from
    // any near-1 corner and makes PickingUp dominate the L3 softmax
    // over Forage within the 16-tick budget.
    set_focal_hunger(world, "Cinder", 0.2);
    spawn_three_carcasses_east(world);
    // Stores building west of the cat — PickingUp's eligibility requires
    // `HasFoodStorageAccessible` (early-game shuffle fix). This scenario
    // isolates the *pickup* leg; deposit-side success is covered by
    // `SCENARIO_TO_STORES`. `FoodStores.current` remains 0 across the
    // 16-tick budget (cats commit to pickup and don't reach deposit),
    // so `food_fraction` stays pinned at 0 and the scavenge-urgency
    // curve still saturates.
    use crate::components::building::{StoredItems, Structure, StructureType};
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        Position::new(16, 20),
    ));
    assert_has_ground_carcass(world);
}

fn setup_picking_up_to_stores(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    // 0.55 — above the kill-resolver's `production_self_eat_threshold`
    // of 0.5 (see `hunt_deposit_chain.rs`). The cat picks up the
    // carcass and proceeds to TravelTo(Stores) → DepositPrey rather
    // than eating in place. `colony_food_security` is still pinned at
    // 0 by empty `FoodStores`, so PickingUp's curve evaluates the
    // same as in the 0.2 case; the difference is downstream
    // post-pickup election.
    set_focal_hunger(world, "Cinder", 0.55);
    // Stores building four tiles west of the cat — directional
    // reversal from the carcasses (all east), so the post-pickup
    // TravelTo(Stores) trip exercises a real spatial chain. Mirrors
    // `hunt_deposit_chain::setup` (Stores at (18, 20), cat at
    // (20, 20), prey east). FoodStores capacity derives from this
    // structure via `sync_food_stores`; current starts at 0.
    use crate::components::building::{StoredItems, Structure, StructureType};
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        Position::new(16, 20),
    ));
    spawn_three_carcasses_east(world);
    assert_has_ground_carcass(world);
}

pub static SCENARIO: Scenario = Scenario {
    name: "picking_up_scavenging",
    default_focal: "Cinder",
    // Empirically tick 1 commits to Wander (a multi-tick fallback plan)
    // before PickingUp's eligibility flows through the L1→L2 pool. The
    // commit suppresses L3 emission for several ticks; the first
    // PickUp election lands around tick 11 in the ~12-tick range. Set
    // to 30 to give the test budget enough headroom to capture ≥1
    // PickUp win across seed-42 noise.
    //
    // Ticket 497 — bumped 16 → 30. Under the post-497 full Chebyshev
    // realignment (specifically the threat-context block's
    // `colony_dist`), the cat reads itself as closer to the colony when
    // wandering diagonally — Chebyshev gives the substrate-correct
    // 8-direction movement distance. Threat-dampening factor shifts;
    // the first PickUp election slips past tick 16 when ambient
    // wildlife (Hawk/Snake spawned at map edges per
    // `wildlife.rs::spawn_wildlife`) is in play. Same family of
    // seed-42 fragility as 494's 200 → 800 bump on the rat-byproduct
    // test.
    //
    // 2026-07-05 — bumped 30 → 60, aligning with the budget the
    // in-file unit tests already use (`run(&SCENARIO, None, Some(60),
    // 42)`). At 30 ticks the gate rode a single softmax draw: PickUp
    // held p≈90% at tick 1 but a marginal float shift (floating
    // `stable` toolchain codegen drift — no sim commit involved)
    // flipped the seeded draw to Forage (p≈10%), whose multi-tick plan
    // consumed the whole budget. At 60 ticks the Forage plan completes,
    // re-election runs, and PickUp lands 3 retrievals — the gate
    // measures the substrate again, not one draw.
    default_ticks: 60,
    setup: setup_picking_up_scavenging,
    // Ticket 198 — substrate-fires gate. PickingUp's resolver writes
    // `ItemRetrieved` on successful pickup; this scenario empirically
    // produces 3 retrievals across seed-42's 16-tick budget. The gate
    // catches a future regression where the curve / eligibility / plan
    // path stays green at the L2/L3 layer but the resolver no longer
    // hits `record_if_witnessed`.
    expected_features: &["ItemRetrieved"],
};

/// 191 — scavenge → travel → deposit pipeline gate. Sister to
/// `hunt_deposit_chain::SCENARIO` (the kill-driven chain). The
/// existence of two sibling scenarios in this file mirrors
/// `hunt_deposit_chain.rs::{SCENARIO, SCENARIO_INJURED}` — same
/// shared spawn helpers, different setup-time configuration.
pub static SCENARIO_TO_STORES: Scenario = Scenario {
    name: "picking_up_to_stores",
    default_focal: "Cinder",
    // 200 ticks — matches `hunt_deposit_chain`'s budget. The full
    // chain is pickup (~1-3 ticks) + TravelTo(Stores) over four
    // tiles (~10-20 ticks) + DepositPrey (1 tick), but the cat may
    // commit to other plans first (Forage at hunger=0.55) before
    // PickingUp wins. The 200-tick budget absorbs that variance.
    // 140 step 7 — 200 → 500 alongside the unit test's bump: arrivals
    // walk the last tile and every leg carries the acceleration ramp.
    default_ticks: 500,
    setup: setup_picking_up_to_stores,
    // ItemRetrieved fires on pickup; deposit success has no Feature
    // (only failure modes — DepositRejected / DepositFailedNoStore /
    // StorageUpgraded — emit). The deposit-side assertion lives in
    // the test below as `final_food_current >= 1.0`.
    expected_features: &["ItemRetrieved"],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// With three OnGround food-Items present, low cat hunger, and
    /// empty FoodStores, the `colony_food_security` axis sits near 0
    /// and the inverted Logistic gives PickingUp a near-1 score.
    /// Eligibility passes via the pre-inserted `HasGroundCarcass`
    /// marker and the post-shuffle-fix `HasFoodStorageAccessible`
    /// marker (authored from the now-spawned Stores building);
    /// PickingUp wins L3 at least once across the budget.
    /// (Other low-tier needs may also fire — Hunting/Foraging — so
    /// the assertion is "at least one win", not "every tick wins".)
    /// Assertion uses the `ItemRetrieved` feature count rather than
    /// L3-winner counts. The PickUp resolver writes `ItemRetrieved`
    /// whenever it consumes a ground item, regardless of whether L3
    /// elected `Action::PickUp` directly or routed through a plan
    /// template that includes a pickup leaf (Forage's plan template
    /// can subsume the pickup-from-ground step). Feature-count
    /// assertions are robust to the schedule-edge L3-winner shifts
    /// each refactor causes — memory:
    /// `learning_bevy_schedule_edge_perturbation`.
    ///
    /// Seed bumped 42 → 13 by ticket 400, budget bumped 16 → 60 by
    /// the shuffle-fix; both adjustments absorb successive schedule-
    /// edge nudges without changing what the scenario tests.
    #[test]
    fn picking_up_wins_with_ground_food_present() {
        let report = run(&SCENARIO, None, Some(120), 42);
        let item_retrieved = report
            .feature_counts
            .get("ItemRetrieved")
            .copied()
            .unwrap_or(0);
        assert!(
            item_retrieved >= 1,
            "PickingUp must run at least once with three OnGround food-Items, \
             a hungry cat, and a reachable Stores (ItemRetrieved feature gates \
             on resolve_pick_up success); feature_counts={:?}",
            report.feature_counts,
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
        // 140 step 12 — gait desires shift movement one tick later and
        // ramp under max_accel; the 60-tick budget left no slack (same
        // shape as the step-6 30->60 bump).
        let report = run(&SCENARIO, None, Some(120), 42);
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

    /// 191 regression: the full scavenge → travel → deposit chain
    /// lands food in `Stores` within 200 ticks. The pre-191 gap was
    /// that 193's regression-proofing stopped at "cat picked up an
    /// item" — it didn't spawn a Stores or confirm the cat actually
    /// completed the kill-equivalent pipeline. Mirrors
    /// `hunt_deposit_chain::pipeline_lands_food_in_stores`. Hunger is
    /// 0.55 (above the kill-resolver's eat-in-place threshold) so the
    /// cat carries the picked-up carcass to Stores rather than eating
    /// it; `colony_food_security` stays pinned at 0 by empty
    /// FoodStores, so PickingUp's curve still saturates ~0.99 and
    /// wins L3 within the 200-tick budget.
    #[test]
    fn pickup_chain_lands_food_in_stores() {
        // 140 step 7 — 200 → 500: arrivals walk the last tile instead of
        // snapping, and every leg carries the acceleration ramp; the
        // full scavenge→travel→deposit chain lands ~tick 350-450 now.
        let report = run(&SCENARIO_TO_STORES, None, Some(500), 13);
        assert!(
            report.final_food_capacity > 0.0,
            "Stores building should contribute capacity (>0); got {}",
            report.final_food_capacity
        );
        assert!(
            report.final_food_current >= 1.0,
            "scavenge→deposit chain should land at least 1 food in Stores \
             within 200 ticks; got {}/{}",
            report.final_food_current,
            report.final_food_capacity
        );
    }
}
