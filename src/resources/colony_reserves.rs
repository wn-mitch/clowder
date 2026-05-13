use bevy_ecs::prelude::*;

/// Ground-truth aggregator of colony-wide reserve resource counts (ticket 308).
///
/// Recomputed each tick by `sync_colony_reserves` from a sum of every cat's
/// `Inventory` slots plus every `Stores` building's `StoredItems`. The per-cat
/// `ColonyReservesBelief` substrate is the **subjective** view of these
/// quantities; this resource is the ground truth that the aggregator emits and
/// that downstream debug / canary code may inspect.
///
/// `RemedyHerb` aggregates `HealingMoss + Moonpetal + Calmroot` — same
/// classification as `Inventory::has_remedy_herb()` and `ResourceKind`.
#[derive(Resource, Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColonyReserves {
    pub thornbriar_count: u32,
    pub remedy_herb_count: u32,
}
