//! `Caretake` — Social-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1509: `WeightedSum` of 3 axes —
//! kitten_urgency, compassion, is_parent. RtEO composition: parent
//! bonus drives low-compassion parents (bloodline override);
//! compassion drives non-parents responding to hungry kittens.
//! `is_parent` is a 0/1 axis — the non-trivial RtEO weight encodes
//! the bloodline-override signal numerically.
//!
//! Ticket 156 — kitten-cry perception is composed at the **modifier
//! layer** (`KittenCryCaretakeLift` in `src/ai/modifier.rs`) rather
//! than as a fourth DSE axis. Initial Phase 4 attempted the latter
//! but a weighted-sum rebalance (legacy weights compressed to make
//! room for the cry axis) reduced the baseline Caretake score by
//! ~40% when no cry was heard, which empirically cut Caretake action
//! count from 56 to 51 in the seed-42 soak. The modifier-layer
//! consumer is purely additive — when cry is heard, Caretake gets a
//! lift on top of the legacy three-axis score; when no cry is heard,
//! the score is bit-identical to the pre-156 baseline.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

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
    pub fn new(scoring: &ScoringConstants) -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // 209: positive `colony_food_security` axis. Plain Logistic
        // (no Invert) — output rises with food security. Default
        // weight 0.0 ships dormant.
        let lift_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        let lift_weight = scoring.caretake_food_security_weight.clamp(0.0, 1.0);
        let remainder = 1.0 - lift_weight;
        Self {
            id: DseId("caretake"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    KITTEN_URGENCY_INPUT,
                    linear.clone(),
                )),
                Consideration::Scalar(ScalarConsideration::new(COMPASSION_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(IS_PARENT_INPUT, linear)),
                Consideration::Scalar(ScalarConsideration::new(
                    "colony_food_security",
                    lift_curve,
                )),
            ],
            // RtEO sum = 1.0. Urgency dominates (hungry kitten is
            // time-sensitive); compassion is the non-parent driver;
            // parent-axis 0/1 carries the bloodline-override signal.
            // The fourth axis (colony_food_security) ships at default-
            // zero weight; the other three scale by `remainder` so
            // the weight sum stays 1.0 even when balance-tuning
            // lifts the lift knob.
            // Cry-perception (ticket 156) lives at the modifier
            // layer — see KittenCryCaretakeLift in src/ai/modifier.rs.
            composition: Composition::weighted_sum(vec![
                0.45 * remainder,
                0.30 * remainder,
                0.25 * remainder,
                lift_weight,
            ]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
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

pub fn caretake_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(CaretakeDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_scoring() -> ScoringConstants {
        ScoringConstants::default()
    }

    #[test]
    fn caretake_dse_id_stable() {
        assert_eq!(CaretakeDse::new(&default_scoring()).id().0, "caretake");
    }

    #[test]
    fn caretake_weights_sum_to_one() {
        let sum: f32 = CaretakeDse::new(&default_scoring())
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn caretake_food_security_dormant_at_default_zero() {
        let s = default_scoring();
        assert_eq!(s.caretake_food_security_weight, 0.0);
        let weights = CaretakeDse::new(&s).composition().weights.clone();
        assert_eq!(weights.len(), 4);
        assert!((weights[0] - 0.45).abs() < 1e-4);
        assert!((weights[1] - 0.30).abs() < 1e-4);
        assert!((weights[2] - 0.25).abs() < 1e-4);
        assert!((weights[3] - 0.0).abs() < 1e-4);
    }
}
