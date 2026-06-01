//! Per-cat L1 substrate: cost-to-reach scalar field.
//!
//! Built once per replan via `flood_dijkstra` (`src/ai/route_cost.rs`).
//! Sampled at L2 by `Consideration::Field` (commit 2) to score
//! destination-aware DSE axes; walked at step-time via
//! `step_along_field` / `CatPathPlan` (commit 10) for gradient-descent
//! routing with A\* fallback.
//!
//! **Substrate, not search state** — see §4.7 of
//! `docs/systems/ai-substrate-refactor.md`. Cat-keyed, in the same
//! family as `escape_viability` and `fox_scent_level` (cat-position
//! scalars), distinct from world-keyed `InfluenceMap` resources.
//! Walker (Brogue, 2010); Mark/Dill (GDC AI Summit influence-map
//! composition); Khatib (1986 potential fields) name this lineage.
//!
//! **Memory.** `width * height * 4` bytes per cat. At 100×100 = 40KB;
//! at 50 adult cats = ~2MB. Acceptable.

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// Sentinel cost for "unreachable / beyond budget / out-of-bounds".
/// Tiles not reached by `flood_dijkstra` retain this value in
/// `costs`. The `Consideration::Field` evaluator collapses any
/// `cost >= MAX_COST_BUDGET` to score 0.0 under closer-is-better
/// curve shapes (mirrors `SpatialConsideration`'s out-of-range
/// behavior).
///
/// Value chosen as `600`: terrain max=4 + per-tile overlay max ≈ 18
/// (fox-scent path-cost max=8 + corruption path-cost max=10) ≈ 22
/// per edge. A 30-tile flood radius at avg-edge ~10 ⇒ ~300 typical;
/// 600 caps the worst case for radii ≤ ~30 tiles. Tunable via
/// `ScoringConstants::route_cost_flood_budget`; this constant is
/// the structural ceiling that bounds bucket-queue allocation.
pub const MAX_COST_BUDGET: u32 = 600;

/// Per-cat scalar field: cost-to-reach every map tile from the cat's
/// position at flood time, under overlay-aware edge weights.
#[derive(Component, Debug, Clone)]
pub struct RouteCostField {
    /// Row-major: `costs[y * width + x]`. Length = `width * height`.
    /// Unreached tiles retain `MAX_COST_BUDGET`.
    pub costs: Vec<u32>,
    pub width: u32,
    pub height: u32,
    /// Cat position at flood time. `cost_at(origin) = 0`.
    pub origin: Position,
    /// Tick at which this field was computed. Read by `CatPathPlan`
    /// (commit 10) to detect staleness against the current sim tick.
    pub origin_tick: u64,
}

impl RouteCostField {
    /// Allocate a field of the given dimensions with every tile at
    /// `MAX_COST_BUDGET` (unreached). The flood populates `costs` in
    /// place.
    pub fn empty(width: u32, height: u32, origin: Position, origin_tick: u64) -> Self {
        let len = (width as usize) * (height as usize);
        Self {
            costs: vec![MAX_COST_BUDGET; len],
            width,
            height,
            origin,
            origin_tick,
        }
    }

    /// Cost to reach `p` from `origin`. Returns `MAX_COST_BUDGET` if
    /// `p` is out of bounds or was not reached by the flood.
    #[inline]
    pub fn cost_at(&self, p: Position) -> u32 {
        if p.x() < 0 || p.y() < 0 {
            return MAX_COST_BUDGET;
        }
        let (xu, yu) = (p.x() as u32, p.y() as u32);
        if xu >= self.width || yu >= self.height {
            return MAX_COST_BUDGET;
        }
        self.costs[(yu * self.width + xu) as usize]
    }

    /// True iff `p` is in bounds and was reached by the flood (i.e.
    /// `cost_at(p) < MAX_COST_BUDGET`).
    #[inline]
    pub fn is_reachable(&self, p: Position) -> bool {
        self.cost_at(p) < MAX_COST_BUDGET
    }
}
