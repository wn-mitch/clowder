use bevy_ecs::prelude::*;

/// Bundles colony-wide optional resources that many scoring systems need.
/// Exists to keep systems under Bevy's 16-param limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct ColonyContext<'w> {
    pub knowledge: Option<Res<'w, crate::resources::colony_knowledge::ColonyKnowledge>>,
    pub priority: Option<Res<'w, crate::resources::colony_priority::ColonyPriority>>,
    pub exploration_map: Res<'w, crate::resources::ExplorationMap>,
    pub fox_scent_map: Res<'w, crate::resources::FoxScentMap>,
    /// 219: colony-shared spatial memory of recent ambush events.
    /// Read at ScoringContext build time in `disposition.rs` and
    /// `goap.rs` to populate `recent_ambush_at_position`. Dormant in
    /// scoring at land (no DSE consumes it yet) but trace-visible via
    /// `ctx_scalars` so soak-trace verification can see the substrate.
    pub recent_ambush_map: Res<'w, crate::resources::RecentAmbushMap>,
    /// 220: per-tile carcass-scent sample. Read at ScoringContext build
    /// time to populate `carcass_scent_at_position`. Substrate is
    /// Phase 2C; this entry restores the perception-scalar consumer
    /// originally scoped in 209 §Scope line 55 (trimmed from the actual
    /// landing). Dormant in DSE scoring; the placement-side consumer
    /// reads `CarcassScentMap` directly from `PlacementMaps`.
    pub carcass_scent_map: Res<'w, crate::resources::CarcassScentMap>,
    /// Ticket 228 — read by the replan-time `LandmarkAnchor::NearestPreyAnchor`
    /// resolver in `evaluate_and_plan` to populate
    /// `CatAnchorPositions.nearest_prey`. Read-only; the replan path
    /// queries via `PreyScentMaps::highest_nearby_any` once per cat
    /// (ticket 062 — max-aggregate across all five per-species sub-maps).
    pub prey_scent_maps: Res<'w, crate::resources::PreyScentMaps>,
    pub cat_scent_map: ResMut<'w, crate::resources::CatScentMap>,
    /// Hearing-channel kitten-cry broadcast (ticket 156). Sampled at
    /// each cat's position to populate `ScoringContext::kitten_cry_perceived`.
    pub kitten_cry_map: Res<'w, crate::resources::KittenCryMap>,
    /// §L2.10.7 colony-wide single-instance building positions
    /// (kitchen, stores, garden). Read by the cat-side
    /// `EvalCtx::anchor_position` closure for `LandmarkAnchor::Nearest{Kitchen,Stores,Garden}`.
    pub colony_landmarks: Res<'w, crate::resources::ColonyLandmarks>,
    /// §L2.10.7 territory-corruption centroid cache. Read by
    /// ColonyCleanse via `LandmarkAnchor::TerritoryCorruptionCentroid`.
    pub corruption_landmarks: Res<'w, crate::resources::CorruptionLandmarks>,
    /// §L2.10.7 colony-center position. Anchors the cat-side
    /// `LandmarkAnchor::TerritoryPerimeterAnchor` and
    /// `NearestPerimeterTile` lookups (perimeter is offset from
    /// colony center).
    pub colony_center: Res<'w, crate::resources::ColonyCenter>,
    /// 256 R3 — ward-coverage influence map. The Patrol DSE's
    /// `TerritoryPerimeterAnchor` resolves to a per-replan rotating
    /// sector centroid over this map (with a fallback to
    /// `colony_center + patrol_perimeter_offset` when no sector has
    /// coverage yet). Read in both the disposition-pipeline scoring
    /// path (`disposition.rs`) and the replan path (`goap.rs`).
    pub ward_coverage_map: Res<'w, crate::resources::WardCoverageMap>,
    /// 301 — coordinator-stamped ward-placement intent. Sampled at
    /// the cat's current position to populate
    /// `ScoringContext::ward_intent_at_position`, which the
    /// `HerbcraftWardDse` reads as a substrate-dormant scalar gated
    /// by `ward_intent_dse_weight` (default 0.0). At default
    /// `SimConstants` the resource is allocated but unwritten; the
    /// sample reads 0.0 everywhere and the dormant weight makes the
    /// DSE score byte-identical pre-301.
    pub ward_intent_map: Res<'w, crate::resources::WardIntentMap>,
    /// 263 — `ActionAffordances` resource borrow for `ScoringContext`
    /// population. The substrate (261) is colony-wide and read by
    /// both production scoring paths (`evaluate_and_plan` and the
    /// currently-unscheduled `evaluate_dispositions`); bundling here
    /// keeps the per-system 16-param ceiling intact for both. Consumer
    /// DSEs at 263 (Flee `flee_affordance`, Hunt per-target
    /// `hunt_best_predation_affordance`) read through
    /// `ctx.action_affordances.read(cat, target, kind)` inside
    /// their consideration closures.
    pub action_affordances: Res<'w, crate::resources::action_affordances::ActionAffordances>,
}

pub mod actions;
pub mod affordance_writer;
pub mod ai;
pub mod aspirations;
pub mod belief_integrator;
pub mod buildings;
pub mod colony_knowledge;
pub mod colony_score;
pub mod combat;
pub mod coordination;
pub mod death;
pub mod disposition;
pub mod fate;
pub mod fertility;
pub mod fox_goap;
pub mod fox_spatial;
pub mod fulfillment;
pub mod goap;
pub mod growth;
pub mod incapacitation;
pub mod influence_map;
pub mod interoception;
pub mod items;
pub mod magic;
pub mod memory;
pub mod mood;
pub mod narrative;
pub mod needs;
pub mod personality_events;
pub mod personality_friction;
pub mod plan_substrate;
pub mod pregnancy;
pub mod prey;
pub mod sensing;
pub mod snapshot;
pub mod social;
pub mod task_chains;
pub mod time;
pub mod trace_emit;
pub mod visitors;
pub mod weather;
pub mod wildlife;
pub mod wind;
