//! `Mate` — Social-urgency peer (§3.3.2 anchor = 1.0). L3 Goal DSE
//! from the §7.M three-layer Mating model (L1 aspiration, L2 pairing
//! activity, L3 `mate_with_goal`).
//!
//! Per §2.3 + §3.1.1 row 1508: `CompensatedProduct` of 2 axes —
//! `mating_deficit` via Logistic(6, 0.6) (reproductive-urgency
//! threshold — seasonal receptivity + cumulative need produce an
//! inflection, not a linear rise) and `warmth` via Linear. Both gate:
//! no drive ⇒ no action; no warmth toward the partner ⇒ the action
//! would not be a valid Mate.
//!
//! Eligibility: `has_eligible_mate` (outer gate until §4 port).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const MATING_DEFICIT_INPUT: &str = "mating_deficit";
pub const WARMTH_INPUT: &str = "warmth";

pub struct MateDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl MateDse {
    pub fn new() -> Self {
        Self {
            id: DseId("mate"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    MATING_DEFICIT_INPUT,
                    Curve::Logistic {
                        steepness: 6.0,
                        midpoint: 0.6,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    WARMTH_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            // §4.3 LifeStage: mating requires Adult or Elder — forbid
            // Kitten and Young as a forward-looking gate for eventual
            // `Without<KittenDependency>` retirement.
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .forbid(markers::Kitten::KEY)
                .forbid(markers::Young::KEY),
        }
    }
}

impl Default for MateDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for MateDse {
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
                label: "mated",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        3
    }
}

pub fn mate_dse() -> Box<dyn Dse> {
    Box::new(MateDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mate_dse_id_stable() {
        assert_eq!(MateDse::new().id().0, "mate");
    }

    #[test]
    fn mate_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            MateDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }
}
