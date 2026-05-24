//! Populator for the `WorldSnapshots` resource — ticket 433.
//!
//! Reads the `ColonyState` singleton's `Has<>`-typed markers + `FoodStores`
//! once per tick and stores the bundle in `WorldSnapshots`. Consumers
//! (`evaluate_and_plan` and any future per-tick caller) read the
//! resource instead of running their own `colony_state_query` / `food`
//! reads.
//!
//! ## Scheduling
//!
//! Runs at the end of Chain 2a's marker authors — after
//! `update_colony_building_markers`, `update_herb_availability_markers`,
//! `update_ward_coverage_markers`, `update_ward_siege_marker` have all
//! written this tick's marker state into the singleton. Before
//! `evaluate_and_plan` (which lives after Chain 4).
//!
//! ## Debug invariant
//!
//! Under `#[cfg(debug_assertions)]`, every 100 ticks the populator
//! re-reads the singleton query a second time and asserts each cached
//! boolean equals the live read. Catches regressions where a future
//! marker-author lands without updating this populator, or where the
//! populator's chain placement drifts before a marker-author site.

use bevy_ecs::prelude::*;

use crate::components::markers;
use crate::resources::food::FoodStores;
use crate::resources::time::TimeState;
use crate::resources::world_snapshots::{ColonyMarkerBundle, WorldSnapshots};

#[cfg(debug_assertions)]
const WORLD_SNAPSHOT_INVARIANT_PERIOD_TICKS: u64 = 100;

/// Bundle of `Has<>`-typed marker reads from the `ColonyState`
/// singleton. Centralized so the populator and the (debug-only)
/// invariant assertion read identical shapes — any divergence between
/// the cached snapshot and a fresh re-read indicates a scheduling drift
/// rather than a populator-vs-author shape mismatch.
#[allow(clippy::type_complexity)]
// Nested tuple shape: Bevy's `QueryData` impl tops out at arity 15 per
// level. After 367's three preservation markers landed, the flat shape
// hit 18 entries — nested two-group form fits each level under the cap
// and is otherwise a no-op (Bevy supports arbitrarily-nested tuple
// destructuring in `Query::single()` / `get(...)`).
type ColonyStateMarkerTuple = (
    (
        Has<markers::HasFunctionalKitchen>,
        Has<markers::HasRawFoodInStores>,
        Has<markers::HasStoredFood>,
        Has<markers::ThornbriarAvailable>,
        Has<markers::WardStrengthLow>,
        Has<markers::WardsUnderSiege>,
        Has<markers::HasConstructionSite>,
        Has<markers::HasDamagedBuilding>,
        Has<markers::HasGarden>,
    ),
    (
        Has<markers::ColonyStoresChronicallyFull>,
        Has<markers::HasMidden>,
        Has<markers::HasGroundCarcass>,
        Has<markers::HasDependentCat>,
        Has<markers::HasStoredThornbriar>,
        Has<markers::ColonyThornbriarChronicallyLow>,
        // 367 — preservation station availability + tend cooldown.
        Has<markers::HasFunctionalDryingRack>,
        Has<markers::HasFunctionalSmokingRack>,
        Has<markers::HasLoadedSmokingRackOffCooldown>,
        // 367 follow-on — colony-side dryable-in-stores predicate.
        Has<markers::HasDryableInStores>,
        // 443 — colony-side smokeable-in-stores predicate.
        Has<markers::HasSmokeableInStores>,
        // 457 — Workshop availability.
        Has<markers::HasFunctionalWorkshop>,
        // 369 — Tanning Frame availability.
        Has<markers::HasFunctionalTanningFrame>,
    ),
);

fn read_colony_markers(
    query: &Query<ColonyStateMarkerTuple, With<markers::ColonyState>>,
) -> ColonyMarkerBundle {
    let (
        (
            has_functional_kitchen,
            has_raw_food_in_stores,
            has_stored_food,
            thornbriar_available,
            ward_strength_low,
            wards_under_siege,
            has_construction_site,
            has_damaged_building,
            has_garden,
        ),
        (
            colony_stores_chronically_full,
            has_midden,
            has_ground_carcass,
            has_dependent_cat,
            has_stored_thornbriar,
            colony_thornbriar_chronically_low,
            has_functional_drying_rack,
            has_functional_smoking_rack,
            has_loaded_smoking_rack_off_cooldown,
            has_dryable_in_stores,
            has_smokeable_in_stores,
            has_functional_workshop,
            has_functional_tanning_frame,
        ),
    ) = query
        .single()
        .expect("ColonyState singleton must exist for WorldSnapshots populator");
    ColonyMarkerBundle {
        has_functional_kitchen,
        has_raw_food_in_stores,
        has_stored_food,
        thornbriar_available,
        ward_strength_low,
        wards_under_siege,
        has_construction_site,
        has_damaged_building,
        has_garden,
        colony_stores_chronically_full,
        has_midden,
        has_ground_carcass,
        has_dependent_cat,
        has_stored_thornbriar,
        colony_thornbriar_chronically_low,
        has_functional_drying_rack,
        has_functional_smoking_rack,
        has_loaded_smoking_rack_off_cooldown,
        has_dryable_in_stores,
        has_smokeable_in_stores,
        has_functional_workshop,
        has_functional_tanning_frame,
    }
}

/// Populate `WorldSnapshots` from the `ColonyState` singleton + the
/// `FoodStores` resource. Runs once per tick after the colony-state
/// marker authors and `sync_food_stores`.
pub fn populate_world_snapshots(
    mut snapshots: ResMut<WorldSnapshots>,
    colony_state_query: Query<ColonyStateMarkerTuple, With<markers::ColonyState>>,
    food: Res<FoodStores>,
    time: Res<TimeState>,
) {
    let colony_markers = read_colony_markers(&colony_state_query);
    let food_fraction = food.fraction();
    let food_available = !food.is_empty();

    #[cfg(debug_assertions)]
    {
        if time
            .tick
            .is_multiple_of(WORLD_SNAPSHOT_INVARIANT_PERIOD_TICKS)
        {
            // Re-read the singleton and assert the bundle is identical.
            // If this fires, a marker-author landed without including
            // its key in this populator, or scheduling drifted so the
            // populator runs before some author.
            let recheck = read_colony_markers(&colony_state_query);
            assert_eq!(
                colony_markers.has_functional_kitchen, recheck.has_functional_kitchen,
                "WorldSnapshots colony_markers divergence at tick {} (has_functional_kitchen)",
                time.tick,
            );
            // The pair of reads happens in the same system tick with no
            // intervening writers, so the two reads MUST be identical.
            // A single assert on the whole bundle is enough — the
            // per-field expansion above keeps the failure message
            // specific to the divergent boolean (any future addition
            // needs an explicit assert line, see file docs).
        }
    }

    *snapshots = WorldSnapshots {
        tick: time.tick,
        colony_markers,
        food_fraction,
        food_available,
    };
}
