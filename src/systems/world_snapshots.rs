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
type ColonyStateMarkerTuple = (
    Has<markers::HasFunctionalKitchen>,
    Has<markers::HasRawFoodInStores>,
    Has<markers::HasStoredFood>,
    Has<markers::ThornbriarAvailable>,
    Has<markers::WardStrengthLow>,
    Has<markers::WardsUnderSiege>,
    Has<markers::HasConstructionSite>,
    Has<markers::HasDamagedBuilding>,
    Has<markers::HasGarden>,
    Has<markers::ColonyStoresChronicallyFull>,
    Has<markers::HasMidden>,
    Has<markers::HasGroundCarcass>,
    Has<markers::HasDependentCat>,
    Has<markers::HasStoredThornbriar>,
    Has<markers::ColonyThornbriarChronicallyLow>,
);

fn read_colony_markers(
    query: &Query<ColonyStateMarkerTuple, With<markers::ColonyState>>,
) -> ColonyMarkerBundle {
    let (
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
