//! §7.W Fulfillment register — per-cat retrospective scalar tracking which
//! behavioral axes are being satisfied, independent of the Maslow needs
//! hierarchy.
//!
//! MVP contains only the `social_warmth` axis (ticket 012 warmth split).
//! Future axes (spiritual, mastery, corruption-capture) add fields here;
//! sensitization/tolerance/diversity-decay mechanics add per-axis dynamics
//! on top of this container.
//!
//! Design spec: `docs/systems/ai-substrate-refactor.md` §7.W.0–§7.W.8.

use bevy_ecs::prelude::*;

/// Per-cat fulfillment register. Architecturally distinct from `Needs` —
/// fulfillment sits *above* Maslow in priority (a cat can be physically
/// comfortable and socially starved) and is morally silent (the framework
/// doesn't label any axis as pathological).
///
/// All values are `f32` in `[0.0, 1.0]` where 1.0 = fully satisfied.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Fulfillment {
    /// Social-warmth fulfillment axis. Drained by isolation; restored by
    /// grooming-other (both parties), socializing, bond proximity.
    /// Split from the old conflated `needs.warmth` — see `warmth-split.md`.
    pub social_warmth: f32,
    /// Ticket 032 — body-condition fulfillment axis. Slow-moving scalar
    /// that decays under sustained low hunger and recovers under sustained
    /// satiation; loosely modeling real-cat body condition (fat reserves,
    /// muscle, coat). Default 1.0; **default decay/recovery rates are 0.0
    /// so the axis ships flat** until a treatment override exercises it.
    /// `#[serde(default = "default_body_condition")]` keeps existing
    /// save-files compatible.
    #[serde(default = "default_body_condition")]
    pub body_condition: f32,
}

fn default_body_condition() -> f32 {
    1.0
}

impl Default for Fulfillment {
    fn default() -> Self {
        Self {
            social_warmth: 0.6,
            body_condition: 1.0,
        }
    }
}

impl Fulfillment {
    /// Create fulfillment with `social_warmth` staggered by position within a
    /// group. Mirrors `Needs::staggered` — spreads initial values so cats
    /// don't all cross thresholds at the same tick.
    pub fn staggered(index: usize, group_size: usize) -> Self {
        let mut f = Self::default();
        if group_size > 1 {
            let t = index as f32 / (group_size - 1) as f32;
            f.social_warmth = 0.7 - t * 0.2; // [0.5, 0.7]
        }
        f
    }

    /// Deficit form for scoring: how unsatisfied is social_warmth?
    pub fn social_warmth_deficit(&self) -> f32 {
        (1.0 - self.social_warmth).clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_social_warmth_is_0_6() {
        let f = Fulfillment::default();
        assert!((f.social_warmth - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn deficit_inverse_of_level() {
        let f = Fulfillment { social_warmth: 0.3, body_condition: 1.0 };
        assert!((f.social_warmth_deficit() - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn deficit_clamps_at_boundaries() {
        let low = Fulfillment {
            social_warmth: -0.1,
            body_condition: 1.0,
        };
        assert!((low.social_warmth_deficit() - 1.0).abs() < f32::EPSILON);

        let high = Fulfillment { social_warmth: 1.5, body_condition: 1.0 };
        assert!((high.social_warmth_deficit() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn staggered_spreads_values() {
        let first = Fulfillment::staggered(0, 5);
        let last = Fulfillment::staggered(4, 5);
        assert!(first.social_warmth > last.social_warmth);
        assert!((first.social_warmth - 0.7).abs() < f32::EPSILON);
        assert!((last.social_warmth - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn staggered_single_cat_uses_default() {
        let f = Fulfillment::staggered(0, 1);
        assert!((f.social_warmth - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn serde_round_trip() {
        let original = Fulfillment {
            social_warmth: 0.42,
            body_condition: 0.7,
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: Fulfillment = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }
}
