//! Drying-chain eligibility microexperiment — ticket 436.
//!
//! Three sister fixtures isolate why `DryFoodDse` is scoring zero in
//! the post-367-Commit-9 verification soak (`logs/tuned-42-5598499f`):
//! the racks are built, `FoodLoadedOnDryingRack` never fires, and
//! `DryFood` does not appear in any cat's `last_scores` over 108k
//! post-build ticks. The soak's signal is "DSE silently filtered";
//! these scenarios answer *which required marker* is the offender by
//! exercising the four `[suspect]` rows of 436's layer-walk one at a
//! time in a preloaded world.
//!
//! The three fixtures cover the eligibility-shape combinations the
//! Commit 9 split-shape fix was supposed to widen:
//!
//! - **hot_inventory** — cat carries a `RawFish`, functional+idle
//!   `DryingRack` at the cat's tile, empty Stores. `HasDryableInInventory`
//!   alone is enough for the `HasDryableAccessible` composite to be
//!   true (left disjunct), so `DryFoodDse` should be eligible.
//! - **stores_has_dryable** — cat's inventory empty, Stores has
//!   one `RawFish` entity stored, functional+idle rack one tile away.
//!   The right disjunct (`has_free_slot && has_dryable_in_stores`)
//!   carries the composite; this is the path Commit 9 introduced.
//! - **empty_stores** — cat's inventory empty, Stores empty,
//!   functional rack present. Neither disjunct holds; `DryFoodDse`
//!   should be eligibility-filtered. Negative-control: confirms the
//!   filter still rejects when nothing is dryable.
//!
//! Reading the report: the binary prints an L2 score-column table
//! per tick. Ineligible rows get `!!` next to the DSE name; eligible
//! ones get blank space. If `dry_food` shows `!!` in fixture 1 or 2,
//! the composite marker (or one of its sub-markers) is silently false
//! and the soak's "DSE never fires" symptom is explained. If `dry_food`
//! shows blank+nonzero score in fixtures 1 and 2 (and is absent or
//! `!!` in 3), eligibility is fine and the failure is downstream
//! (scoring, planning, or resolver).
//!
//! Per CLAUDE.md "Scenario microexperiment before a soak" — answering
//! this with `just soak` would burn 15 minutes per iteration; the
//! scenario answers it in ~3 seconds.

use bevy_ecs::world::World;

use crate::components::building::{DryingRackState, StoredItems, Structure, StructureType};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::Inventory;
use crate::components::physical::{Needs, Position};
use crate::components::Personality;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position { x: 20, y: 20 };
const RACK_POS: Position = Position { x: 21, y: 20 };
const STORES_POS: Position = Position { x: 19, y: 20 };

/// Common tick budget. We only need eligibility to settle, which takes
/// 1–2 ticks (one for the colony-marker writer to fire, one for
/// `evaluate_and_plan` to read it back). A handful of extra ticks lets
/// us see whether the chosen action is stable.
const DEFAULT_TICKS: u32 = 10;

pub static SCENARIO_HOT_INVENTORY: Scenario = Scenario {
    name: "drying_chain_hot_inventory",
    default_focal: "Cinder",
    default_ticks: DEFAULT_TICKS,
    setup: setup_hot_inventory,
    expected_features: &[],
};

pub static SCENARIO_STORES_HAS_DRYABLE: Scenario = Scenario {
    name: "drying_chain_stores_has_dryable",
    default_focal: "Cinder",
    default_ticks: DEFAULT_TICKS,
    setup: setup_stores_has_dryable,
    expected_features: &[],
};

pub static SCENARIO_EMPTY_STORES: Scenario = Scenario {
    name: "drying_chain_empty_stores",
    default_focal: "Cinder",
    default_ticks: DEFAULT_TICKS,
    setup: setup_empty_stores,
    expected_features: &[],
};

// Ticket 439 fixtures 4–5: resolver completion. The 436/437 trio
// settled eligibility; the post-437 soak surfaced a *next-layer*
// defect — `TravelTo(DryingRack): no reachable zone target × 1095` and
// SmokingRack × 719 while `FoodLoadedOnDryingRack` never fires. The
// layer-walk found marker-writer and zone-resolver predicates aligned
// on `site.is_none() && condition > 0.2`, which mechanically demands
// the slice be non-empty whenever the marker fires — yet the failure
// requires the slice empty at step-exec. These two fixtures isolate
// whether the basic resolver chain completes happy-path at unit scale.
// Drying takes 15_000 ticks (`drying_dried_fish_total_ticks`), so we
// only assert `FoodLoadedOnDryingRack` (the load step's witness), not
// `FoodDried`.
const FAR_CAT_POS: Position = Position { x: 5, y: 5 };
const FAR_RACK_POS: Position = Position { x: 35, y: 35 };

/// Resolver budget: enough ticks for the cat to elect DryFood, plan,
/// TravelTo the 1-tile-away rack, and execute the DryFood load step.
const RESOLVER_NEAR_TICKS: u32 = 30;
/// Far-rack budget: ~60 tiles Manhattan, A* path-follow at ~1 tile/tick,
/// plus the load step. 90 leaves slack for re-plans.
const RESOLVER_FAR_TICKS: u32 = 90;

// `expected_features` left empty: the canonical seed-42 softmax draw at
// tick 0 lands on Forage for ~30% of the L3 pool's probability mass
// (DryFood scores highest in L2 but only wins ~70%), so the
// `declared_expected_features_all_fire` integration test in
// `tests/scenarios.rs` (which runs every scenario at seed=42) can't
// gate on `FoodLoadedOnDryingRack` deterministically. The unit tests
// below use a seed where DryFood elects (probed via
// `diagnostic_probe_seeds_for_dryfood_election`) and assert directly.
pub static SCENARIO_RESOLVER_COMPLETES: Scenario = Scenario {
    name: "drying_chain_resolver_completes",
    default_focal: "Cinder",
    default_ticks: RESOLVER_NEAR_TICKS,
    setup: setup_resolver_completes,
    expected_features: &[],
};

pub static SCENARIO_RESOLVER_FAR_RACK: Scenario = Scenario {
    name: "drying_chain_resolver_far_rack",
    default_focal: "Cinder",
    default_ticks: RESOLVER_FAR_TICKS,
    setup: setup_resolver_far_rack,
    expected_features: &[],
};

// ---------------------------------------------------------------------
// Fixture 1: hot inventory
// ---------------------------------------------------------------------
//
// One adult cat holding `RawFish` in slot 0. A functional+idle
// `DryingRack` sits at the cat's tile. An (empty) Stores exists so the
// FoodStores resource resolves a non-zero capacity (some scoring axes
// read it). The expected behavior: `HasDryableInInventory` fires on
// the cat → left disjunct of `HasDryableAccessible` is true →
// `DryFoodDse` eligibility passes. The composite-marker path on the
// right disjunct is *not* exercised by this fixture; it's exercised
// by fixture 2.
fn setup_hot_inventory(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_drying_rack(world, RACK_POS);
    spawn_empty_stores(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_dryfood_baseline_needs),
    );
    fill_focal_inventory_with_raw_fish(world, "Cinder", 1);
}

// ---------------------------------------------------------------------
// Fixture 2: empty inventory + stores has dryable
// ---------------------------------------------------------------------
//
// One adult cat with an empty inventory adjacent to a functional+idle
// `DryingRack`. A Stores entity at (19,20) holds one `RawFish` Item
// entity in its `StoredItems` list. Expected behavior:
// `HasDryableInInventory` is *false* on the cat (left disjunct
// fails), but `inventory.is_full() == false` AND
// `HasDryableInStores` is true (right disjunct holds) →
// `HasDryableAccessible` true → `DryFoodDse` eligibility passes.
// This is the Commit 9 split-shape path. If this fixture shows
// `dry_food` as ineligible, Commit 9's composite-marker writer at
// `goap.rs:1981` is the failing row.
fn setup_stores_has_dryable(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_drying_rack(world, RACK_POS);
    spawn_stores_with_raw_fish(world, STORES_POS, 1);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_dryfood_baseline_needs),
    );
}

// ---------------------------------------------------------------------
// Fixture 3: empty inventory + empty stores
// ---------------------------------------------------------------------
//
// Same layout as fixture 2 but the Stores is empty. Neither disjunct
// of `HasDryableAccessible` holds. Negative control: `DryFoodDse`
// should be eligibility-filtered. If this row shows `eligible: true`
// the composite marker is leaky (returning true when nothing is
// dryable) — the inverse failure mode of fixtures 1 & 2.
fn setup_empty_stores(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_drying_rack(world, RACK_POS);
    spawn_empty_stores(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_dryfood_baseline_needs),
    );
}

// ---------------------------------------------------------------------
// Fixture 4: resolver-completes happy path
// ---------------------------------------------------------------------
//
// Same shape as `setup_hot_inventory` but the runner ticks long enough
// for the cat to elect, plan, travel one tile, and execute the DryFood
// load step. If the load fires (`FoodLoadedOnDryingRack` records),
// the resolver chain is structurally intact and the soak's failure is
// state-specific (rack-destruction or snapshot staleness, not the
// resolver logic). If it doesn't fire, we've reproduced the soak bug
// at unit scale and can drill into `goap.rs:8266`'s upstream.
fn setup_resolver_completes(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_drying_rack(world, RACK_POS);
    spawn_empty_stores(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", COLONY_CENTER)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_dryfood_baseline_needs)
            .with_personality(bias_personality_toward_drying),
    );
    fill_focal_inventory_with_raw_fish(world, "Cinder", 1);
}

// ---------------------------------------------------------------------
// Fixture 5: resolver-completes with a far rack (A* exercise)
// ---------------------------------------------------------------------
//
// Cat at (5,5), rack at (35,35) — 60 tiles Manhattan, traversal under
// A* gradient-walk. Tests whether the failure mode is reachability or
// terrain-bound rather than zone-table emptiness. If Fixture 4 passes
// and this one fails, the bug is in `find_full_path` / `next_step`,
// not in `resolve_zone_position`.
fn setup_resolver_far_rack(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_drying_rack(world, FAR_RACK_POS);
    spawn_empty_stores(world, STORES_POS);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Cinder", FAR_CAT_POS)
            .with_marker(MarkerKind::Adult)
            .with_needs(set_dryfood_baseline_needs)
            .with_personality(bias_personality_toward_drying),
    );
    fill_focal_inventory_with_raw_fish(world, "Cinder", 1);
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// Spawn a fully-functional, idle `DryingRack` structure at `pos`.
/// Skips the construction-site path (`Structure::new(DryingRack)` ships
/// `condition: 1.0` → `effectiveness() == 1.0` → satisfies the
/// `update_colony_building_markers` gate at `buildings.rs:712`).
fn spawn_drying_rack(world: &mut World, pos: Position) {
    world.spawn((
        Structure::new(StructureType::DryingRack),
        DryingRackState::default(),
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

/// Spawn a Stores building containing `count` `RawFish` Item entities.
/// Items are spawned with `ItemLocation::StoredIn(stores)` and no
/// Position component — matches the production deposit shape
/// (`resolve_deposit_at_stores:141-148`). Required for the
/// `HasDryableInStores` colony marker to fire (`buildings.rs:537-548`).
fn spawn_stores_with_raw_fish(world: &mut World, pos: Position, count: usize) {
    let stores = world
        .spawn((
            Structure::new(StructureType::Stores),
            StoredItems::default(),
            pos,
        ))
        .id();
    for _ in 0..count {
        let item = world
            .spawn(Item::new(
                ItemKind::RawFish,
                1.0,
                ItemLocation::StoredIn(stores),
            ))
            .id();
        let mut em = world.entity_mut(stores);
        let mut stored = em
            .get_mut::<StoredItems>()
            .expect("Stores must have StoredItems");
        stored.items.push(item);
    }
}

fn fill_focal_inventory_with_raw_fish(world: &mut World, focal_name: &str, count: usize) {
    use crate::components::identity::Name;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist before fill_focal_inventory_with_raw_fish");
    let mut em = world.entity_mut(entity);
    let mut inv = em.get_mut::<Inventory>().expect("focal has Inventory");
    for _ in 0..count {
        inv.add_item(ItemKind::RawFish);
    }
}

/// Set Needs to a profile where DryFood has a fighting chance against
/// other tier-2 DSEs but Eat / Sleep don't dominate. Hunger 0.7 sits
/// well above the acute-Eat band but inside the `scarcity()` curve's
/// productive midrange so the food_scarcity axis carries some signal.
/// Energy 0.9 keeps Sleep out of the running.
fn set_dryfood_baseline_needs(n: &mut Needs) {
    n.hunger = 0.7;
    n.energy = 0.9;
}

/// Ticket 439: bias personality to make `DryFoodDse` the deterministic
/// L3 winner against sibling tier-2 DSEs (Forage, Cook, Wander) in the
/// resolver-completion fixtures. DryFood's WeightedSum weights
/// diligence at 0.24 and reads it as `diligence_value * 1.0`; pushing
/// diligence to 0.95 adds ~0.11 to DryFood's score relative to a
/// balanced 0.5. Curiosity low keeps Forage's exploration axis down.
fn bias_personality_toward_drying(p: &mut Personality) {
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

    /// Helper: walk the report for the first tick whose L2 table
    /// includes a row for `dry_food`. Returns the row's `eligible`
    /// flag and `final_score`. None if `dry_food` never surfaced —
    /// itself a useful diagnostic (registry mis-wired, or scoring
    /// pass elided the DSE).
    fn first_dry_food_row(
        report: &crate::scenarios::runner::ScenarioReport,
    ) -> Option<(bool, f32)> {
        for t in &report.ticks {
            if let Some(row) = t.l2.iter().find(|r| r.dse == "dry_food") {
                return Some((row.eligible, row.final_score));
            }
        }
        None
    }

    /// Fixture 1 expectation: cat with RawFish in inventory makes the
    /// left disjunct of `HasDryableAccessible` true, so `dry_food` is
    /// eligible. Regression gate for ticket 437's dispatch fix in
    /// `src/ai/scoring.rs::score_actions` — pre-437 the DSE was
    /// registered but never scored, so no L2 row surfaced at all.
    #[test]
    fn hot_inventory_makes_dry_food_eligible() {
        let report = run(&SCENARIO_HOT_INVENTORY, None, Some(DEFAULT_TICKS), 42);
        let (eligible, score) = first_dry_food_row(&report).unwrap_or_else(|| {
            panic!(
                "dry_food never surfaced in any L2 table — DSE not scoring at all. \
                 Check populate_dse_registry. Ticks captured: {}",
                report.ticks.len()
            )
        });
        assert!(
            eligible,
            "dry_food should be eligible when cat has RawFish in inventory \
             (left disjunct of HasDryableAccessible). Got eligible=false, \
             final_score={score}. This narrows the failure to the per-cat \
             HasDryableInInventory writer or the composite-marker read in goap.rs."
        );
    }

    /// Diagnostic dump of every relevant marker on the `ColonyState`
    /// singleton + focal cat after fixture 2 settles. Useful as a
    /// next-layer-audit tool when the eligibility assertions below
    /// fail and the test message can't identify which required marker
    /// is missing. Always-`#[ignore]`d; not part of the CI gate.
    /// Run with: `cargo test -- --ignored
    /// scenarios::drying_chain_eligibility::tests::diagnostic_dump_marker_state_fixture_2`
    #[test]
    #[ignore = "diagnostic — run manually when eligibility tests fail to identify the missing marker"]
    fn diagnostic_dump_marker_state_fixture_2() {
        use crate::components::identity::Name;
        use crate::components::markers as m;
        use crate::scenarios::runner::build_scenario_app;

        let mut app = build_scenario_app(42, &SCENARIO_STORES_HAS_DRYABLE, "Cinder");
        // Drive the same warm-up as `run()` does.
        app.update();
        // Five ticks is plenty for all colony markers to settle.
        for _ in 0..5 {
            app.update();
        }
        let colony = {
            let mut q = app
                .world_mut()
                .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<m::ColonyState>>(
                );
            let w = app.world();
            q.iter(w).next().expect("ColonyState singleton missing")
        };
        let world = app.world();
        eprintln!("--- ColonyState markers (fixture 2) ---");
        eprintln!(
            "  HasFunctionalDryingRack:           {}",
            world
                .entity(colony)
                .contains::<m::HasFunctionalDryingRack>()
        );
        eprintln!(
            "  HasDryableInStores:                {}",
            world.entity(colony).contains::<m::HasDryableInStores>()
        );
        eprintln!(
            "  HasRawFoodInStores:                {}",
            world.entity(colony).contains::<m::HasRawFoodInStores>()
        );
        eprintln!(
            "  HasFunctionalKitchen:              {}",
            world.entity(colony).contains::<m::HasFunctionalKitchen>()
        );

        // Cat markers. Release the read borrow before issuing the
        // next mutable query.
        let _ = world;
        let cat = {
            let mut q = app.world_mut().query::<(bevy_ecs::entity::Entity, &Name)>();
            let w = app.world();
            q.iter(w)
                .find(|(_, n)| n.0 == "Cinder")
                .map(|(e, _)| e)
                .expect("focal cat missing")
        };
        let world = app.world();
        eprintln!("--- Cat markers (fixture 2) ---");
        eprintln!(
            "  Adult:                             {}",
            world.entity(cat).contains::<m::Adult>()
        );
        eprintln!(
            "  CanDry:                            {}",
            world.entity(cat).contains::<m::CanDry>()
        );
        eprintln!(
            "  CanCook:                           {}",
            world.entity(cat).contains::<m::CanCook>()
        );
        eprintln!(
            "  HasDryableInInventory:             {}",
            world.entity(cat).contains::<m::HasDryableInInventory>()
        );
        eprintln!(
            "  HasFreeSlot:                       {}",
            world.entity(cat).contains::<m::HasFreeSlot>()
        );
        eprintln!(
            "  Incapacitated:                     {}",
            world.entity(cat).contains::<m::Incapacitated>()
        );
        eprintln!(
            "  Injured:                           {}",
            world.entity(cat).contains::<m::Injured>()
        );
    }

    /// Fixture 2 expectation: empty inventory + Stores with RawFish
    /// makes the right disjunct of `HasDryableAccessible` true (cat
    /// has a free slot AND colony has dryable in stores), so
    /// `dry_food` is eligible. This is the Commit 9 split-shape path.
    /// Same regression-gate role as fixture 1: exercises the
    /// composite-marker right disjunct rather than the simpler
    /// inventory-only path.
    #[test]
    fn stores_has_dryable_makes_dry_food_eligible_via_composite() {
        let report = run(&SCENARIO_STORES_HAS_DRYABLE, None, Some(DEFAULT_TICKS), 42);
        let (eligible, score) = first_dry_food_row(&report).unwrap_or_else(|| {
            panic!(
                "dry_food never surfaced in any L2 table. \
                 Ticks captured: {}",
                report.ticks.len()
            )
        });
        assert!(
            eligible,
            "dry_food should be eligible via the Commit 9 composite-marker path \
             (empty inv + colony has dryable in Stores). Got eligible=false, \
             final_score={score}. This is the load-bearing assertion for \
             ticket 436 — fixture 2 firing means the soak-time silent filtering \
             is the HasDryableAccessible composite. Inspect the \
             HasDryableInStores colony marker (buildings.rs:537-548) and the \
             inventory.is_full() projection at goap.rs:1980."
        );
    }

    /// Fixture 3 expectation: nothing dryable anywhere → composite is
    /// false → `dry_food` ineligibility-filtered. Negative control.
    /// If `dry_food` shows up *eligible* here, the composite is leaky
    /// (returning true with empty inputs) and the soak-time
    /// over-elections of DryFood are downstream of a composite bug,
    /// not under-elections.
    #[test]
    fn empty_stores_filters_dry_food() {
        let report = run(&SCENARIO_EMPTY_STORES, None, Some(DEFAULT_TICKS), 42);
        match first_dry_food_row(&report) {
            None => {
                // L2 capture path may elide ineligible DSEs entirely.
                // Either shape is acceptable: absent = filtered, or
                // present-with-eligible-false = filtered.
            }
            Some((eligible, score)) => assert!(
                !eligible,
                "dry_food should be filtered when nothing is dryable anywhere. \
                 Got eligible=true, final_score={score}. The composite marker \
                 HasDryableAccessible is leaky — verify the right-disjunct \
                 short-circuit at goap.rs:1981 doesn't return true when \
                 has_dryable_in_stores is false."
            ),
        }
    }

    /// Diagnostic: scan seeds 1–50 and report which give DryFood as the
    /// L3 softmax draw on tick 0. Helpful when picking a seed for the
    /// resolver-completion fixtures (the seed needs to actually elect
    /// DryFood, not just rank it highest).
    #[test]
    #[ignore = "diagnostic — manual probe for fixture seed selection"]
    fn diagnostic_probe_seeds_for_dryfood_election() {
        for seed in 1u64..50 {
            let report = run(&SCENARIO_RESOLVER_COMPLETES, None, None, seed);
            let chosen_t0 = report.ticks.first().and_then(|t| t.chosen.clone());
            let loaded = report
                .feature_counts
                .get("FoodLoadedOnDryingRack")
                .copied()
                .unwrap_or(0);
            eprintln!("seed={seed} tick0={chosen_t0:?} FoodLoadedOnDryingRack={loaded}");
        }
    }

    /// Diagnostic: build the scenario app with a DryFood-electing seed,
    /// tick the full resolver budget, and dump the `EventLog` plan-
    /// failure tallies + `SystemActivation` feature counts. Used during
    /// ticket 439's investigation to confirm the load step's witness
    /// fired. Run with `cargo test -- --ignored
    /// scenarios::drying_chain_eligibility::tests::diagnostic_dump_resolver_outcome`.
    #[test]
    #[ignore = "diagnostic — re-run if resolver completion assertions regress"]
    fn diagnostic_dump_resolver_outcome() {
        use crate::resources::event_log::EventLog;
        use crate::scenarios::runner::build_scenario_app;
        let mut app = build_scenario_app(
            RESOLVER_FIXTURE_SEED,
            &SCENARIO_RESOLVER_COMPLETES,
            "Cinder",
        );
        app.update();
        for _ in 0..RESOLVER_NEAR_TICKS {
            app.update();
        }
        let world = app.world();
        let log = world.resource::<EventLog>();
        eprintln!("plan_failures_by_reason: {:?}", log.plan_failures_by_reason);
        if let Some(act) =
            world.get_resource::<crate::resources::system_activation::SystemActivation>()
        {
            let mut keys: Vec<_> = act.counts.iter().filter(|(_, c)| **c > 0).collect();
            keys.sort_by_key(|(_, c)| -(**c as i64));
            for (f, c) in keys.iter().take(15) {
                eprintln!("  feature {f:?} count={c}");
            }
        }
    }

    /// Ticket 439 Fixture 4: cat with RawFish adjacent to a built+idle
    /// DryingRack should elect DryFood, walk one tile, and execute the
    /// load step within 30 ticks. The witness is
    /// `Feature::FoodLoadedOnDryingRack` firing ≥1× (drying itself
    /// takes 15k ticks; the load is the first observable step). If
    /// this assertion fires, the basic resolver chain is intact and
    /// the post-437 soak failure is a state-specific phenomenon — the
    /// next step is to author Fixture 6 (rack-destruction-mid-plan).
    /// If it doesn't fire, the bug is structural and we drill into
    /// `goap.rs:8266`'s upstream.
    /// Seed selected via `diagnostic_probe_seeds_for_dryfood_election`
    /// — DryFood deterministically wins the softmax draw at tick 0 here.
    /// Pre-100 used seed 1; ticket 100 added `tremor_tick` to the
    /// per-tick chain and inserted Stalk/Pounce into the Action enum,
    /// which perturbed the seed-1 softmax-RNG state (precedent:
    /// `learning_bevy_schedule_edge_perturbation`) and pushed the
    /// far-rack 90-tick window onto a Forage/Wander track. Re-probed
    /// for the post-100 schedule: seeds 21/22/25/26/31/38–41/43/47–49
    /// elect DryFood at tick 0 AND complete the far-rack load within
    /// budget. Seed 21 is the first in that intersection; the L3 pool
    /// is well-defined across all probes (DryFood scores highest in
    /// L2), the seed merely picks which softmax bucket lands.
    const RESOLVER_FIXTURE_SEED: u64 = 21;

    #[test]
    fn resolver_completes_load_step_on_adjacent_rack() {
        let report = run(
            &SCENARIO_RESOLVER_COMPLETES,
            None,
            None,
            RESOLVER_FIXTURE_SEED,
        );
        let loaded = report
            .feature_counts
            .get("FoodLoadedOnDryingRack")
            .copied()
            .unwrap_or(0);
        assert!(
            loaded >= 1,
            "FoodLoadedOnDryingRack should fire when a cat with RawFish stands \
             adjacent to a built+idle DryingRack and ticks for {RESOLVER_NEAR_TICKS} \
             at seed {RESOLVER_FIXTURE_SEED}. Got {loaded}. This is the load-bearing \
             assertion for ticket 439 — pre-fix the snapshot path between scoring \
             (goap.rs:1733, via `WorldStateQueries.building_query`) and step-exec \
             (goap.rs:3813, via `BuildingResolverParams.buildings`) diverged: the \
             step-exec query filtered out `With<DryingRackState>` for borrow-checker \
             disjointness, so `drying_rack_positions` was always empty for rack \
             entities. The fix at goap.rs:3728 chains rack entries from \
             `building_params.drying_racks`/`smoking_racks` into the snapshot. \
             Winner counts across run: {:?}",
            report.winner_counts()
        );
    }

    /// Ticket 439 Fixture 5: same shape but rack is ~60 tiles away.
    /// Exercises A*'s `find_full_path` + greedy fallback. If Fixture 4
    /// passes and this fails, the failure mode is reachability —
    /// `resolve_zone_position` returns a position the cat can't path
    /// to. If both pass, the soak failure is genuinely state-specific.
    #[test]
    fn resolver_completes_load_step_on_far_rack() {
        let report = run(
            &SCENARIO_RESOLVER_FAR_RACK,
            None,
            None,
            RESOLVER_FIXTURE_SEED,
        );
        let loaded = report
            .feature_counts
            .get("FoodLoadedOnDryingRack")
            .copied()
            .unwrap_or(0);
        assert!(
            loaded >= 1,
            "FoodLoadedOnDryingRack should fire even when the rack is \
             ~60 tiles away, given a {RESOLVER_FAR_TICKS}-tick budget. \
             Got {loaded}. If Fixture 4 passed but this didn't, the failure \
             is in A* / `find_full_path` — `resolve_zone_position` returns a \
             position the cat can't path to. Winner counts: {:?}",
            report.winner_counts()
        );
    }
}
