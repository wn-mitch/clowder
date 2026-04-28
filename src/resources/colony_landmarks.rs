//! Per-tick colony-building landmark cache — §L2.10.7 anchor resolution.
//!
//! Holds the positions of single-instance colony buildings (kitchen,
//! stores, garden) used by self-state DSEs as `LandmarkAnchor` targets.
//! Populated each tick by `update_colony_landmarks` in
//! `systems/buildings.rs`, which scans `Structure` entities once per
//! tick rather than each scoring builder repeating the scan.
//!
//! Read by the cat-side `EvalCtx::anchor_position` closure (cat
//! `score_dse_by_id`) for:
//!
//! - `LandmarkAnchor::NearestKitchen` — Cook (B3), HerbcraftPrepare (B10).
//! - `LandmarkAnchor::NearestStores` — Eat (B4).
//! - `LandmarkAnchor::NearestGarden` — Farm (B5).
//!
//! Also read by the fox-side closure for `NearestVisibleStore` (C20)
//! when the fox's per-tick visibility check confirms the store is in
//! range.
//!
//! **Why a separate resource and not inline scans.** Each
//! `ScoringContext` builder runs once per cat per tick; scanning all
//! `Structure` entities for the kitchen would mean N×M work where N
//! is cats and M is buildings. The colony has at most ~20 buildings
//! and ~50 cats — running the scan once per tick into a cached
//! resource and reading it 50× costs O(M) instead of O(N·M).

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// Per-tick cached positions of single-instance colony buildings.
/// Each field is `None` when no instance of that building exists.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ColonyLandmarks {
    pub kitchen: Option<Position>,
    pub stores: Option<Position>,
    pub garden: Option<Position>,
}

impl ColonyLandmarks {
    /// Empty landmarks — all fields `None`. Useful in tests where no
    /// building scan has run.
    pub fn empty() -> Self {
        Self::default()
    }
}
