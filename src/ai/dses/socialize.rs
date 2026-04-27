//! `Socialize` — Social-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of 6 axes — the highest-n
//! RtEO DSE in the catalog aside from Fight. Loneliness-anchor
//! Logistic on social_deficit; sociability + temper + playfulness
//! personality coefficients; inverted-need-penalty on phys_sat
//! (bilinear temper × (1 - phys_sat) lives in composition); and a
//! threshold-gated corruption pushback bonus.
//!
//! Maslow tier 2 — matches the old inline `level_suppression(2)`.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{inverted_need_penalty, loneliness, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const SOCIAL_DEFICIT_INPUT: &str = "social_deficit";
pub const SOCIABILITY_INPUT: &str = "sociability";
pub const TEMPER_INPUT: &str = "temper";
pub const PHYS_SATISFACTION_INPUT: &str = "phys_satisfaction";
pub const PLAYFULNESS_INPUT: &str = "playfulness";
pub const TILE_CORRUPTION_INPUT: &str = "tile_corruption";
pub const SOCIAL_WARMTH_DEFICIT_INPUT: &str = "social_warmth_deficit";

pub struct SocializeDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SocializeDse {
    pub fn new() -> Self {
        // Corruption bonus: threshold gate at 0.1 absorbed into the
        // curve per §2.3 row 1025. Logistic(8, 0.1) maps
        // tile_corruption < 0.1 to near-0 and above to positive.
        let corruption_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.1,
        };
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("socialize"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SOCIAL_DEFICIT_INPUT, loneliness())),
                Consideration::Scalar(ScalarConsideration::new(SOCIABILITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(TEMPER_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(
                    PHYS_SATISFACTION_INPUT,
                    inverted_need_penalty(),
                )),
                Consideration::Scalar(ScalarConsideration::new(PLAYFULNESS_INPUT, linear)),
                Consideration::Scalar(ScalarConsideration::new(
                    TILE_CORRUPTION_INPUT,
                    corruption_curve,
                )),
                // §7.W: social_warmth fulfillment deficit. Small
                // additive rider — socializing addresses social_warmth
                // at a slower rate than grooming, so the weight is
                // lower.
                Consideration::Scalar(ScalarConsideration::new(
                    SOCIAL_WARMTH_DEFICIT_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.1,
                    },
                )),
            ],
            // RtEO sum = 1.0. Loneliness dominates; sociability +
            // playfulness are secondary personality drivers. Temper
            // and phys_sat inverted-penalty jointly express the old
            // `temper × (1-phys_sat)` subtraction as two positive
            // additive axes (both high ⇒ strong modulation of the
            // score downward via the non-social axes dominating).
            // Corruption bonus is a small-weight additive rider.
            // Social-warmth deficit (0.08) is a gentle nudge.
            composition: Composition::weighted_sum(vec![0.32, 0.19, 0.05, 0.09, 0.19, 0.08, 0.08]),
            // §13.1: `.forbid(markers::Incapacitated::KEY)` blocks downed cats.
            // §9.3 stance binding migrated to `socialize_target_dse` —
            // candidate-prefilter happens in the target-taking resolver,
            // not on the cat-action DSE which has no candidate set.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for SocializeDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for SocializeDse {
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
        // §7.3: Socializing → OpenMinded. Activity-shaped; drops on
        // sated-sociability or lost interest.
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "socialized",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn socialize_dse() -> Box<dyn Dse> {
    Box::new(SocializeDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socialize_dse_id_stable() {
        assert_eq!(SocializeDse::new().id().0, "socialize");
    }

    #[test]
    fn socialize_has_seven_axes() {
        assert_eq!(SocializeDse::new().considerations().len(), 7);
    }

    #[test]
    fn socialize_weights_sum_to_one() {
        let sum: f32 = SocializeDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn socialize_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            SocializeDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn socialize_maslow_tier_is_two() {
        assert_eq!(SocializeDse::new().maslow_tier(), 2);
    }

}
