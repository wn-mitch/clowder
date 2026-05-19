//! 084 Commit 3 — chronicity tracking for the
//! `ColonyThornbriarChronicallyLow` marker.
//!
//! Mirrors `StoresPressureTracker`'s shape but inverts the polarity:
//! `latched_chronic` is `true` iff the colony's *stash level* has
//! stayed below `ScoringConstants::thornbriar_stash_low_threshold`
//! across a full `chronicity_window_ticks` window. The 176 tracker
//! counted *event deltas* (DepositRejected accumulations); this one
//! samples *state* (current stash sum) — the difference is structural:
//! we care about prolonged absence of supply, not the rate of any
//! event class.
//!
//! Single-point sampling at window boundaries is sufficient because
//! deposit/retrieve flow against the stash is order-of-magnitude
//! slower than the chronicity window (a 1000-tick window holds ~16
//! sim-seconds of activity, and a herb gather/retrieve cycle is
//! multi-second). If the stash drops below threshold and stays there
//! across the window's two endpoints, calling it "chronically low" is
//! conservative; if it bounces above between samples we'd want the
//! marker to clear — and the next sample correctly catches that.

use bevy_ecs::resource::Resource;

#[derive(Resource, Debug, Clone, Default)]
pub struct ThornbriarPressureTracker {
    /// Tick when the current window started. The next sample fires at
    /// `last_window_tick + chronicity_window_ticks`.
    pub last_window_tick: u64,
    /// Latched verdict from the most recent window. Read by
    /// `update_colony_building_markers` between window boundaries so
    /// the marker stays stable; only window-boundary ticks recompute.
    pub latched_chronic: bool,
}
