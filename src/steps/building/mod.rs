mod construct;
pub mod deliver;
mod gather;
mod harvest;
mod move_to;
mod pickup_material;
mod repair;
mod tend;

pub use construct::resolve_construct;
pub use deliver::resolve_deliver;
pub use gather::resolve_gather;
pub use harvest::resolve_harvest;
pub use move_to::resolve_move_to;
pub use pickup_material::resolve_pickup_material;
pub use repair::resolve_repair;
pub use tend::resolve_tend;

/// Filter tuple shared by every building-resolver function's
/// `buildings` query parameter. Bundles `Without<TaskChain>`
/// (cat/building disjointness) with `Without<DryingRackState>` /
/// `Without<SmokingRackState>` (ticket 367 — keeps the building
/// resolvers' mutable `&Structure` access statically disjoint from
/// `BuildingResolverParams.drying_racks` / `.smoking_racks`, both
/// of which read `&Structure` on preservation-rack archetypes only).
/// When a new preservation-station state Component lands, append
/// its `Without<...>` here and every `resolve_*` picks it up
/// without further edits.
pub type BuildingsResolverFilter = (
    bevy::ecs::query::Without<crate::components::task_chain::TaskChain>,
    bevy::ecs::query::Without<crate::components::building::DryingRackState>,
    bevy::ecs::query::Without<crate::components::building::SmokingRackState>,
);
