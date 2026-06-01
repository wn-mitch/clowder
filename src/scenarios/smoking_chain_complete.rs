//! Smoking-chain end-to-end completion — ticket 447.
//!
//! Restores deterministic regression coverage for the smoking pipeline
//! after ticket 444 retired `MeatLoadedOnSmokingRack` /
//! `SmokingRackTended` / `MeatSmoked` from the per-soak never-fired-
//! positives canary. Under healthy seed-42 colony shape the meat-AND-
//! fuel conjunction inside `HasSmokeableAccessible` never resolves
//! (446's verified layer-walk root cause), so the canary fires
//! spuriously — but a refactor that breaks one of the three Features'
//! `record_if_witnessed` calls in `goap.rs::dispatch_step_action`
//! would now go undetected at soak time. This fixture preloads the
//! state the organic colony never delivers (one Adult cat carrying
//! `RawMouse` + `Wood` adjacent to a functional idle SmokingRack) and
//! ticks long enough for the full chain to fire all three Features.
//!
//! Sister to `drying_chain_resolver_completes` (ticket 439). Unlike
//! drying (15k-tick completion → only the `FoodLoadedOnDryingRack`
//! load step is asserted), smoking completes in
//! `smoking_tends_needed × smoking_tend_cooldown_ticks` = 3 × 416 ≈
//! 1248 tend-cycle ticks (sim_constants.rs:6688/6698), so we can
//! assert all three Features within a single fixture's tick budget.
//!
//! `expected_features: &[]` — opts out of the seed-42 integration
//! gate (`tests/scenarios.rs::declared_expected_features_all_fire`).
//! Rationale matches the drying-completes fixtures: even with
//! personality biased toward smoke_meat, the L3 softmax at seed 42
//! can land in a sibling DSE's bucket. The unit test below pins a
//! seed where `SmokeMeat` deterministically wins at tick 0 (probed
//! via `diagnostic_probe_seeds_for_smoke_meat_election`).

use bevy_ecs::world::World;

use crate::components::building::{SmokingRackState, StoredItems, Structure, StructureType};
use crate::components::items::ItemKind;
use crate::components::magic::Inventory;
use crate::components::personality::Personality;
use crate::components::physical::{Needs, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position::new(20, 20);
const RACK_POS: Position = Position::new(21, 20);
const STORES_POS: Position = Position::new(19, 20);

/// Tick budget: covers the 1-tile travel, the single-tick load, three
/// tend cycles spaced `smoking_tend_cooldown_ticks=416` apart (~1248
/// ticks), and ~750 ticks of headroom for re-planning, sleep
/// interrupts, and the L3 softmax occasionally drawing a sibling DSE
/// before settling back on SmokeMeat.
const DEFAULT_TICKS: u32 = 2000;

pub static SCENARIO: Scenario = Scenario {
    name: "smoking_chain_complete",
    default_focal: "Cinder",
    default_ticks: DEFAULT_TICKS,
    setup,
    expected_features: &[],
};

// ---------------------------------------------------------------------
// Setup: cat with meat + fuel adjacent to a functional idle SmokingRack
// ---------------------------------------------------------------------
//
// Mirrors `drying_chain_eligibility::setup_resolver_completes` exactly,
// substituting smoking-side items + structure. One Adult cat at
// (20,20) carrying RawMouse + Wood in inventory, a functional idle
// SmokingRack at (21,20), and an empty Stores at (19,20) so the
// FoodStores resource resolves a non-zero capacity. Personality
// biased toward smoking via the diligence axis (mirroring the
// drying-completes bias) so `SmokeMeat` dominates the L2 ranking.
fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_smoking_rack(world, RACK_POS);
    spawn_empty_stores(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_smokemeat_baseline_needs)
            .with_personality(bias_personality_toward_smoking),
    );
    fill_focal_inventory_with_smokeable(world, "Cinder");
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// Spawn a fully-functional, idle `SmokingRack` at `pos`.
/// `Structure::new(SmokingRack)` ships `condition: 1.0` →
/// `effectiveness() == 1.0` → satisfies the
/// `HasFunctionalSmokingRack` colony-marker gate.
fn spawn_smoking_rack(world: &mut World, pos: Position) {
    world.spawn((
        Structure::new(StructureType::SmokingRack),
        SmokingRackState::default(),
        pos,
    ));
}

/// Spawn a Stores building with an empty `StoredItems` list. Required
/// for the FoodStores resource to derive a non-zero capacity in
/// `sync_food_stores`; otherwise `food_scarcity` reads a degenerate 0/0.
fn spawn_empty_stores(world: &mut World, pos: Position) {
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        pos,
    ));
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

/// Set Needs to a profile where SmokeMeat has a fighting chance against
/// other tier-2 DSEs but Eat / Sleep don't dominate. Hunger 0.7 sits
/// well above the acute-Eat band but inside the `scarcity()` curve's
/// productive midrange so the food_scarcity axis carries some signal.
/// Energy 0.9 keeps Sleep out of the running.
fn set_smokemeat_baseline_needs(n: &mut Needs) {
    n.hunger = 0.7;
    n.energy = 0.9;
}

/// Bias personality so `SmokeMeatDse` is the deterministic L3 winner
/// against sibling tier-2 DSEs (Forage, Cook, Wander). Mirrors the
/// drying-completes bias profile — SmokeMeat shares the preservation-
/// pipeline scoring shape with DryFood, so the same personality axes
/// dominate. Diligence 0.95 lifts the WeightedSum diligence axis,
/// curiosity 0.1 keeps Forage's exploration axis down.
fn bias_personality_toward_smoking(p: &mut Personality) {
    p.diligence = 0.95;
    p.curiosity = 0.1;
    p.ambition = 0.8;
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// Diagnostic: scan seeds 1–50 and report which give SmokeMeat as
    /// the L3 softmax draw on tick 0 plus how many of the three
    /// Features fired across the run. Used to pick `FIXTURE_SEED`
    /// below. Run with
    /// `cargo test --release -- --ignored
    ///  scenarios::smoking_chain_complete::tests::diagnostic_probe_seeds`.
    #[test]
    #[ignore = "diagnostic — manual probe for fixture seed selection"]
    fn diagnostic_probe_seeds_for_smoke_meat_election() {
        for seed in 1u64..50 {
            let report = run(&SCENARIO, None, None, seed);
            let chosen_t0 = report.ticks.first().and_then(|t| t.chosen.clone());
            let load = report
                .feature_counts
                .get("MeatLoadedOnSmokingRack")
                .copied()
                .unwrap_or(0);
            let tend = report
                .feature_counts
                .get("SmokingRackTended")
                .copied()
                .unwrap_or(0);
            let done = report
                .feature_counts
                .get("MeatSmoked")
                .copied()
                .unwrap_or(0);
            eprintln!(
                "seed={seed} tick0={chosen_t0:?} \
                 MeatLoadedOnSmokingRack={load} \
                 SmokingRackTended={tend} \
                 MeatSmoked={done}"
            );
        }
    }

    /// Seed selected via `diagnostic_probe_seeds_for_smoke_meat_election`.
    /// SmokeMeat deterministically elects at tick 0 here and all three
    /// Features fire within `DEFAULT_TICKS`. The L3 softmax is
    /// stochastic across seeds; SmokeMeat scores near the top of L2 in
    /// the broad seed pool, the seed merely picks which softmax bucket
    /// the draw lands in (matches the drying-completes pattern).
    const FIXTURE_SEED: u64 = 1;

    /// Load-bearing regression assertion for ticket 447. Replaces the
    /// per-soak never-fired-positives gate that ticket 444 retired for
    /// the smoking triple. If any of these three assertions fails, a
    /// recent change broke the resolver chain — drill into the layer
    /// the failing Feature names:
    ///
    /// - `MeatLoadedOnSmokingRack` fails → check
    ///   `goap.rs::dispatch_step_action::SmokeMeat` (the load
    ///   dispatcher at ~line 7855) and
    ///   `src/steps/disposition/load_smoking_rack.rs`. Either the
    ///   `HasSmokeableAccessible` eligibility filter went south, the
    ///   plan template stopped emitting the LoadSmokingRack step, or
    ///   `record_if_witnessed` was removed.
    /// - `SmokingRackTended` fails → load fired but no tend did.
    ///   Check `goap.rs::dispatch_step_action::TendSmokingRack` and
    ///   `src/steps/disposition/tend_smoking_rack.rs`. The
    ///   `HasLoadedSmokingRackOffCooldown` marker may have gone
    ///   silent, or the cooldown predicate is keeping the rack
    ///   permanently un-tendable.
    /// - `MeatSmoked` fails → tends fired but completion didn't.
    ///   The `outcome.witness == Some(true)` branch in
    ///   `goap.rs:7889` is the load-bearing emit; ensure the tend
    ///   resolver still returns `Some(true)` on the final tend (cf.
    ///   `tend_smoking_rack.rs:60` rustdoc).
    #[test]
    fn resolver_completes_full_smoke_chain() {
        let report = run(&SCENARIO, None, None, FIXTURE_SEED);
        let load = report
            .feature_counts
            .get("MeatLoadedOnSmokingRack")
            .copied()
            .unwrap_or(0);
        let tend = report
            .feature_counts
            .get("SmokingRackTended")
            .copied()
            .unwrap_or(0);
        let done = report
            .feature_counts
            .get("MeatSmoked")
            .copied()
            .unwrap_or(0);
        assert!(
            load >= 1,
            "MeatLoadedOnSmokingRack should fire when an Adult cat carrying \
             RawMouse + Wood stands adjacent to a built+idle SmokingRack and \
             ticks for {DEFAULT_TICKS} at seed {FIXTURE_SEED}. Got load={load}, \
             tend={tend}, done={done}. The load step is the first observable \
             witness in the smoking chain — if it didn't fire, the failure is \
             at the LoadSmokingRack dispatcher in goap.rs (~line 7855) or the \
             `HasSmokeableAccessible` eligibility filter. Winner counts: {:?}",
            report.winner_counts()
        );
        assert!(
            tend >= 1,
            "SmokingRackTended should fire on every successful tend cycle \
             (intermediate + completion). Got load={load}, tend={tend}, \
             done={done}. Load fired but tend didn't — drill into \
             `goap.rs::dispatch_step_action::TendSmokingRack` and \
             `src/steps/disposition/tend_smoking_rack.rs`. The \
             `HasLoadedSmokingRackOffCooldown` marker or the \
             `smoking_tend_cooldown_ticks={}` predicate may be keeping the \
             rack permanently un-tendable. Winner counts: {:?}",
            crate::resources::sim_constants::SimConstants::default()
                .crafting
                .smoking_tend_cooldown_ticks,
            report.winner_counts()
        );
        assert!(
            done >= 1,
            "MeatSmoked should fire on the completion tend (the tend cycle \
             that flips `outcome.witness` to `Some(true)`). Got load={load}, \
             tend={tend}, done={done}. Tends fired but completion didn't — \
             either the tick budget is insufficient (3 × \
             smoking_tend_cooldown_ticks = ~1248 ticks; budget is \
             {DEFAULT_TICKS}), or the completion-witness branch at \
             goap.rs:7889 stopped emitting. Check that the tend resolver \
             still returns `Some(true)` on the final tend per \
             `tend_smoking_rack.rs:60` rustdoc. Winner counts: {:?}",
            report.winner_counts()
        );
    }
}
