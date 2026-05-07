//! `PrevSafetyDeficit` — per-cat snapshot of last tick's safety deficit
//! (ticket 108, Phase 2 of the `ThreatProximityAdrenalineFlee` substrate).
//!
//! Stores the previous tick's `safety_deficit = 1 - needs.safety` so this
//! tick's `threat_proximity_derivative = max(0, safety_deficit_now -
//! prev)` can be computed without sampling history elsewhere. The
//! derivative powers the §3.5.1 `ThreatProximityAdrenalineFlee`
//! Modifier's lurch-on-rising-threat trigger — adrenaline is a
//! change-detection signal, not a steady-state threshold.
//!
//! ## Lifecycle
//!
//! - **Spawned-cat insert**: included in `spawn_cat_from_blueprint`'s
//!   bundle (`src/plugins/setup.rs`) initialized to `Default::default()`
//!   = `0.0`. First-tick derivative for a freshly-spawned cat is
//!   therefore `safety_deficit_now - 0.0` — which is fine because cats
//!   spawn at full safety (`needs.safety ≈ 1.0`), so the deficit and
//!   the derivative are both ≈ 0 at spawn.
//! - **Save-loaded cats**: lazy-insert path on first read (mirrors the
//!   `RecentTargetFailures` pattern for pre-component saves). When the
//!   ScoringContext builder finds `None`, it treats prev as "current"
//!   — i.e. derivative = 0 for that tick — and the writeback system
//!   inserts the component on its next pass.
//!
//! ## Read site
//!
//! `evaluate_and_plan` and `evaluate_dispositions` (the two
//! ScoringContext construction sites) read `Option<&PrevSafetyDeficit>`
//! from the cat query and compute the derivative inline before passing
//! it into `ScoringContext.threat_proximity_derivative`. That field
//! flows through `ctx_scalars` as `"threat_proximity_derivative"` for
//! the modifier pipeline. The 118 preempt path
//! (`check_modifier_preemption` in `goap.rs`) also reads it via the
//! same fetch closure.
//!
//! ## Write site
//!
//! `update_prev_safety_deficit` (registered in `SimulationPlugin::build`
//! after the scoring pass) writes `safety_deficit_now` into this
//! component each tick. Schedule placement matters: the writeback must
//! run *after* the scoring read, so the derivative is `now - last_tick`
//! and not `now - now`.

use bevy_ecs::prelude::*;

/// Per-cat snapshot of last tick's `safety_deficit = 1 - needs.safety`
/// in `[0, 1]`. See the module docs for placement and lifecycle.
#[derive(Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct PrevSafetyDeficit(pub f32);

impl PrevSafetyDeficit {
    pub fn new(deficit: f32) -> Self {
        Self(deficit.clamp(0.0, 1.0))
    }

    /// Compute the rising-only derivative `max(0, now - prev)`. Falling
    /// safety deficit (threat receding) yields 0 — adrenaline doesn't
    /// fire on relief. When the component is missing (lazy-insert
    /// case), pass `prev = now` to get a 0 derivative for this tick.
    pub fn rising_derivative(now: f32, prev: f32) -> f32 {
        (now - prev).max(0.0).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_derivative_clamps_to_zero_when_falling() {
        assert_eq!(PrevSafetyDeficit::rising_derivative(0.3, 0.7), 0.0);
    }

    #[test]
    fn rising_derivative_returns_positive_delta() {
        let delta = PrevSafetyDeficit::rising_derivative(0.6, 0.4);
        assert!((delta - 0.2).abs() < 1e-6, "expected 0.2, got {delta}");
    }

    #[test]
    fn rising_derivative_clamps_above_one() {
        assert_eq!(PrevSafetyDeficit::rising_derivative(2.0, 0.0), 1.0);
    }

    #[test]
    fn rising_derivative_zero_when_steady() {
        assert_eq!(PrevSafetyDeficit::rising_derivative(0.5, 0.5), 0.0);
    }

    #[test]
    fn new_clamps_input_range() {
        assert_eq!(PrevSafetyDeficit::new(-0.3).0, 0.0);
        assert_eq!(PrevSafetyDeficit::new(1.7).0, 1.0);
        assert_eq!(PrevSafetyDeficit::new(0.4).0, 0.4);
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(PrevSafetyDeficit::default().0, 0.0);
    }
}
