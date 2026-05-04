//! `Caretake` — Social-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1509: `WeightedSum` of 4 axes —
//! kitten_cry_perceived, kitten_urgency, compassion, is_parent.
//!
//! - `kitten_cry_perceived` (ticket 156) reads the Hearing-channel
//!   `KittenCryMap` at the cat's tile. The cry is threshold-gated on
//!   the kitten's hunger so this axis only fires when a nearby kitten
//!   is critically hungry — that's exactly when Caretake should
//!   dominate, including for non-parent adults out of the
//!   `IsParentOfHungryKitten` marker's parent-only reach.
//! - `kitten_urgency` (per-cat caretake_resolution.urgency) is the
//!   in-engine non-spatial urgency.
//! - `compassion` (caretake-local, bond-scaled) drives non-parents.
//! - `is_parent_of_hungry_kitten` is the 0/1 bloodline-override.
//!
//! RtEO composition: cry dominates when fired so non-parents pivot
//! to Caretake on perceptible distress; the legacy three axes
//! continue to work when no cry is painted (e.g., parent-of-quiet-
//! kitten responding to early urgency).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

/// Cry-broadcast perception axis (ticket 156). Reads
/// `ScoringContext::kitten_cry_perceived` — the Hearing-channel
/// `KittenCryMap` sample at the cat's tile.
pub const KITTEN_CRY_PERCEIVED_INPUT: &str = "kitten_cry_perceived";
pub const KITTEN_URGENCY_INPUT: &str = "kitten_urgency";
/// Caretake-local compassion axis (Phase 4c.4 alloparenting Reframe A).
/// `ctx_scalars` populates this as `personality.compassion ×
/// caretake_compassion_bond_scale`, clamped [0, 1]. The baseline
/// `"compassion"` axis stays shared with herbcraft_prepare — Caretake
/// gets its own key so bond-weighting only amplifies care-for-hungry-
/// kitten decisions, not unrelated compassion-gated actions.
pub const COMPASSION_INPUT: &str = "caretake_compassion";
pub const IS_PARENT_INPUT: &str = "is_parent_of_hungry_kitten";

pub struct CaretakeDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CaretakeDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        Self {
            id: DseId("caretake"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    KITTEN_CRY_PERCEIVED_INPUT,
                    linear.clone(),
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    KITTEN_URGENCY_INPUT,
                    linear.clone(),
                )),
                Consideration::Scalar(ScalarConsideration::new(COMPASSION_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(IS_PARENT_INPUT, linear)),
            ],
            // RtEO sum = 1.0. Cry-perception dominates when fired
            // (Hearing-channel kitten distress is the most direct
            // signal "a kitten near me is critically hungry"); the
            // legacy three axes continue to fire when no cry is
            // painted (parent-of-quiet-kitten early urgency).
            composition: Composition::weighted_sum(vec![0.40, 0.25, 0.20, 0.15]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for CaretakeDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CaretakeDse {
    fn id(&self) -> DseId {
        self.id
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "kitten_fed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        // Caretake is a care-for-offspring action — tier 3 (Love/Belonging).
        3
    }
}

pub fn caretake_dse() -> Box<dyn Dse> {
    Box::new(CaretakeDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caretake_dse_id_stable() {
        assert_eq!(CaretakeDse::new().id().0, "caretake");
    }

    #[test]
    fn caretake_weights_sum_to_one() {
        let sum: f32 = CaretakeDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}
