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

    /// Ticket 452 — newborn-kitten spawn value. Encodes "newborn arrives
    /// in a maximally-bonded post-gestation state": social_warmth high
    /// because gestation is the most maternally-saturated window of a
    /// cat's life (in utero, then nursing-from-birth, mother grooming).
    /// body_condition full-for-stage. The bank decays over the early
    /// weeks if maternal contact drops — the welfare problem is *loss*
    /// of maternal presence, not absence at birth. Distinct from
    /// `GroomingCondition` (which spawns low — newborn coat is dirty
    /// with birth membrane and requires maternal cleaning).
    pub fn newborn() -> Self {
        Self {
            social_warmth: 0.9,
            body_condition: 1.0,
        }
    }

    /// Ticket 488 — founder spawn value. Mirrors `newborn()`'s
    /// architectural reasoning: founders arrive at the colony site
    /// from a prior established social context, not from isolation,
    /// so their `social_warmth` bank should reflect that history.
    /// Without this, founders spawn 30-50% socially-warmth-deficient
    /// via the `staggered` [0.5, 0.7] range — and the
    /// `GroomOtherDse.social_warmth_deficit` axis honestly responds by
    /// driving day-1 chain-grooming dominance (the "cuddle puddle"
    /// that ticket 487 narrowed at the eligibility/resolver layer
    /// without touching the SELF need driver). Pairs with b24d333b's
    /// warm-floor founder `Relationships` init — same architectural
    /// fiction ("not strangers, came from somewhere"), applied to
    /// the second substrate bank that encodes that fiction.
    ///
    /// `[0.85, 1.0]` staggered the same shape as `staggered()` so
    /// per-cat phase offset is preserved (avoids same-tick mass-
    /// threshold-crossings — the original `staggered` rationale).
    /// Single-cat group falls back to 0.95 (mid-range), mirroring
    /// `staggered`'s `default()` fallback shape.
    pub fn founder(index: usize, group_size: usize) -> Self {
        let social_warmth = if group_size > 1 {
            let t = index as f32 / (group_size - 1) as f32;
            1.0 - t * 0.15 // [1.0, 0.85]
        } else {
            0.95
        };
        Self {
            social_warmth,
            body_condition: 1.0,
        }
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
        let f = Fulfillment {
            social_warmth: 0.3,
            body_condition: 1.0,
        };
        assert!((f.social_warmth_deficit() - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn deficit_clamps_at_boundaries() {
        let low = Fulfillment {
            social_warmth: -0.1,
            body_condition: 1.0,
        };
        assert!((low.social_warmth_deficit() - 1.0).abs() < f32::EPSILON);

        let high = Fulfillment {
            social_warmth: 1.5,
            body_condition: 1.0,
        };
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
    fn newborn_has_high_social_warmth_and_full_body_condition() {
        let f = Fulfillment::newborn();
        assert!((f.social_warmth - 0.9).abs() < f32::EPSILON);
        assert!((f.body_condition - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn founder_spawns_with_low_social_warmth_deficit() {
        // Ticket 488 — every founder in a typical clowder must spawn
        // with `social_warmth_deficit <= 0.15` so the
        // `GroomOtherDse.social_warmth_deficit` consideration
        // contributes at floor (Linear(1.0, 0.1) → ~0.25), not as a
        // moderate-driver (~0.5 under the pre-488 staggered
        // [0.3, 0.5] deficit range). Below the floor, the day-1
        // GroomOther chain-grooming pressure has no SELF-state push.
        for n in [1usize, 5, 10] {
            for i in 0..n {
                let f = Fulfillment::founder(i, n);
                assert!(
                    f.social_warmth_deficit() <= 0.15,
                    "founder(i={i}, n={n}) deficit={} > 0.15",
                    f.social_warmth_deficit()
                );
            }
        }
    }

    #[test]
    fn founder_staggers_across_group() {
        // Stagger preserved (avoids same-tick mass-threshold-crossings
        // — the original `staggered` rationale). First and last cat
        // in a 5-cat clowder must differ.
        let first = Fulfillment::founder(0, 5);
        let last = Fulfillment::founder(4, 5);
        assert!(
            first.social_warmth > last.social_warmth,
            "founder stagger lost: first={} last={}",
            first.social_warmth,
            last.social_warmth
        );
        assert!((first.social_warmth - 1.0).abs() < f32::EPSILON);
        assert!((last.social_warmth - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn founder_single_cat_uses_midpoint() {
        // Mirror the `staggered_single_cat_uses_default` shape:
        // single-cat group has no spread, so use a mid-range value
        // rather than the boundary.
        let f = Fulfillment::founder(0, 1);
        assert!((f.social_warmth - 0.95).abs() < f32::EPSILON);
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
