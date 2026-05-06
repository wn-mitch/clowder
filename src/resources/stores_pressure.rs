//! 176 — chronicity tracking for the
//! `ColonyStoresChronicallyFull` marker.
//!
//! Maintains a per-colony rolling baseline of the cumulative
//! `Feature::DepositRejected` count. Each `chronicity_window_ticks`
//! ticks the bookkeeping system in `src/systems/buildings.rs`
//! computes the per-window delta, divides by the colony cat-count,
//! and compares against `chronicity_threshold`. The verdict is
//! latched on this resource so off-window-boundary scoring still
//! sees a stable marker rather than flickering with every tick of
//! activation increments.

use bevy_ecs::resource::Resource;

#[derive(Resource, Debug, Clone, Default)]
pub struct StoresPressureTracker {
    /// `SystemActivation::counts[Feature::DepositRejected]` snapshot
    /// at the start of the current window. Subtract this from the
    /// current cumulative count at window-end to get window
    /// rejections.
    pub last_window_baseline: u64,
    /// Tick when the current window started. The next snapshot fires
    /// at `last_window_tick + chronicity_window_ticks`.
    pub last_window_tick: u64,
    /// Latched verdict from the most recent window. `update_colony_building_markers`
    /// reads this between window boundaries so the marker stays
    /// stable; only window-boundary ticks recompute.
    pub latched_chronic: bool,
}
