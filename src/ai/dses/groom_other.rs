//! `Groom(other)` — sibling-DSE split from the retiring `Max`-composed
//! cat `Groom` inline block (§L2.10.10). Allogrooming — bond-building
//! through physical contact.
//!
//! Per §2.3 rows 1026–1028 + §3.1.1 row 1484: Social-urgency peer
//! (§3.3.2 anchor = 1.0). `CompensatedProduct` of loneliness-anchor
//! social_deficit, Linear warmth personality axis, and the
//! inverted-need-penalty temper modulator (reused from Socialize).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{inverted_need_penalty, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const SOCIAL_DEFICIT_INPUT: &str = "social_deficit";
pub const WARMTH_INPUT: &str = "warmth";
pub const PHYS_SATISFACTION_INPUT: &str = "phys_satisfaction";
pub const SOCIAL_WARMTH_DEFICIT_INPUT: &str = "social_warmth_deficit";

pub struct GroomOtherDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl GroomOtherDse {
    pub fn new() -> Self {
        Self {
            id: DseId("groom_other"),
            considerations: vec![
                // Gentler than the Socialize-shared `loneliness()` (Logistic
                // midpoint 0.7). Real cats groom each other proactively as a
                // bonding behavior, not just when desperately lonely.
                // Midpoint 0.3 means moderate social deficits (~0.2-0.4)
                // produce meaningful scores under the CompensatedProduct.
                Consideration::Scalar(ScalarConsideration::new(
                    SOCIAL_DEFICIT_INPUT,
                    Curve::Logistic {
                        steepness: 8.0,
                        midpoint: 0.3,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    WARMTH_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    PHYS_SATISFACTION_INPUT,
                    inverted_need_penalty(),
                )),
                // §7.W: social_warmth fulfillment deficit. 0.1 floor
                // so groom_other isn't zeroed when social_warmth is
                // full — cats still groom for relationship/social
                // reasons. Lower weight (0.6) so it contributes
                // without dominating the primary social-deficit drive.
                Consideration::Scalar(ScalarConsideration::new(
                    SOCIAL_WARMTH_DEFICIT_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.1,
                    },
                )),
            ],
            // RtM weights: social_deficit, warmth (personality),
            // phys_satisfaction, social_warmth_deficit. The fourth
            // axis at 0.6 contributes meaningfully but doesn't
            // dominate the primary loneliness signal.
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0, 0.6]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for GroomOtherDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for GroomOtherDse {
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
        // §7.3: GroomOther is a constituent action of the Socializing
        // disposition and rides Socializing's `OpenMinded` strategy.
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "groomed_other",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn groom_other_dse() -> Box<dyn Dse> {
    Box::new(GroomOtherDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groom_other_dse_id_stable() {
        assert_eq!(GroomOtherDse::new().id().0, "groom_other");
    }

    #[test]
    fn groom_other_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            GroomOtherDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn groom_other_maslow_tier_is_two() {
        assert_eq!(GroomOtherDse::new().maslow_tier(), 2);
    }
}
