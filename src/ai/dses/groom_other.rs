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
use crate::ai::curves::{inverted_need_penalty, loneliness, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const SOCIAL_DEFICIT_INPUT: &str = "social_deficit";
pub const WARMTH_INPUT: &str = "warmth";
pub const PHYS_SATISFACTION_INPUT: &str = "phys_satisfaction";

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
                Consideration::Scalar(ScalarConsideration::new(SOCIAL_DEFICIT_INPUT, loneliness())),
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
            ],
            // RtM weights [1.0, 1.0, 1.0]: all three axes gate. No
            // lonely signal, no warmth, or high-phys-satisfaction
            // (so low penalty) — any can zero the score.
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid("Incapacitated"),
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
