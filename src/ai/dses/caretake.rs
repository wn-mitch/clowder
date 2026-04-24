//! `Caretake` — Social-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1509: `WeightedSum` of 3 axes —
//! kitten_urgency, compassion, is_parent. RtEO composition: parent
//! bonus drives low-compassion parents (bloodline override);
//! compassion drives non-parents responding to hungry kittens.
//! `is_parent` is a 0/1 axis — the non-trivial RtEO weight encodes
//! the bloodline-override signal numerically.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

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
                    KITTEN_URGENCY_INPUT,
                    linear.clone(),
                )),
                Consideration::Scalar(ScalarConsideration::new(COMPASSION_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(IS_PARENT_INPUT, linear)),
            ],
            // RtEO sum = 1.0. Urgency dominates (hungry kitten is
            // time-sensitive); compassion is the non-parent driver;
            // parent-axis 0/1 carries the bloodline-override signal.
            composition: Composition::weighted_sum(vec![0.45, 0.30, 0.25]),
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
