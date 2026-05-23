//! Cross-system per-tick aggregates — ticket 433 (the rescoped Stage F
//! of 431).
//!
//! ## Purpose
//!
//! Several per-tick systems independently re-derive the same colony
//! aggregates: the `ColonyState` singleton's `Has<>`-typed markers,
//! `FoodStores::fraction()`, the food-availability boolean. The audit
//! captured in `docs/systems/world-snapshots.md` walks the duplications.
//!
//! `WorldSnapshots` collapses the per-tick reads into one resource
//! populated at the end of Chain 2a (after every marker-author system
//! runs) by `populate_world_snapshots`. Consumers downstream of Chain 2a
//! read the resource instead of running their own
//! `colony_state_query.single()` calls or `food.fraction()` reads.
//!
//! ## Why a resource (vs more nested SystemParams)
//!
//! The colony-state marker booleans don't carry meaningful semantics in
//! ECS (each is a ZST on a singleton entity). The substrate is
//! "compute-once values that change at most once per tick." A resource
//! is the substrate-of-record for that shape; threading the booleans
//! through bundles of `Res<Marker>`-shaped readers fragments the
//! ownership for no win.
//!
//! ## Adding new fields
//!
//! See `docs/systems/world-snapshots.md` § "Pattern for future hoists"
//! for the audit / temporal-alignment / debug-invariant checklist that
//! gates new field additions.

use bevy_ecs::prelude::Resource;

/// Cached colony-wide marker booleans. Read once per tick from the
/// `ColonyState` singleton's `Has<>`-typed marker components in
/// `populate_world_snapshots`; consumers in Chain 2b+ read the bundle
/// instead of running their own `colony_state_query`.
///
/// Field names mirror the marker type names exactly so future additions
/// don't require renaming. The substrate-of-record for each boolean
/// remains the marker on the `ColonyState` entity — this bundle is a
/// per-tick read-only mirror.
#[derive(Debug, Default, Clone, Copy)]
pub struct ColonyMarkerBundle {
    pub has_functional_kitchen: bool,
    pub has_raw_food_in_stores: bool,
    pub has_stored_food: bool,
    pub thornbriar_available: bool,
    pub ward_strength_low: bool,
    pub wards_under_siege: bool,
    pub has_construction_site: bool,
    pub has_damaged_building: bool,
    pub has_garden: bool,
    pub colony_stores_chronically_full: bool,
    pub has_midden: bool,
    pub has_ground_carcass: bool,
    pub has_dependent_cat: bool,
    pub has_stored_thornbriar: bool,
    pub colony_thornbriar_chronically_low: bool,
    /// 367 — preservation-station availability mirrors.
    pub has_functional_drying_rack: bool,
    pub has_functional_smoking_rack: bool,
    pub has_loaded_smoking_rack_off_cooldown: bool,
    /// 367 follow-on — colony has ≥1 RawFish or RawOrgan in
    /// `StoredItems`. Used to author the per-cat composite
    /// `HasDryableAccessible` that gates `DryFoodDse` eligibility
    /// even when the cat's inventory is empty.
    pub has_dryable_in_stores: bool,
    /// 443 — colony has ≥1 raw-meat item AND ≥1 fuel (Wood) in
    /// `StoredItems`. Used to author the per-cat composite
    /// `HasSmokeableAccessible` that gates `SmokeMeatDse` eligibility
    /// even when the cat's inventory is empty.
    pub has_smokeable_in_stores: bool,
    /// 457 — ≥1 functional Workshop exists in the colony. Read by
    /// `evaluate_and_plan` to gate the per-cat `CraftAtWorkshopDse`
    /// eligibility filter via `MarkerSnapshot::has(...)`.
    pub has_functional_workshop: bool,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct WorldSnapshots {
    /// Sim tick at population. Debug consumers MAY assert
    /// `snapshot.tick == time.tick` to catch ordering bugs.
    pub tick: u64,

    /// Colony-wide marker booleans cached from the `ColonyState`
    /// singleton (`ticket 168` + `169` + `171` author set).
    pub colony_markers: ColonyMarkerBundle,

    /// `FoodStores::fraction()` snapshot. Computed after `sync_food_stores`
    /// in Chain 1's items pass.
    pub food_fraction: f32,

    /// `!FoodStores::is_empty()` — substrate-of-record for the
    /// `HasStoredFood` marker the food-availability boolean drives.
    pub food_available: bool,
}
