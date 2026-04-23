//! `Explore` — Exploration-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1501: `CompensatedProduct` of 2 axes —
//! curiosity (Linear) + unexplored_nearby (Linear — already a
//! bounded coverage fraction). Both gate.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const CURIOSITY_INPUT: &str = "curiosity";
pub const UNEXPLORED_NEARBY_INPUT: &str = "unexplored_nearby";

pub struct ExploreDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ExploreDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        Self {
            id: DseId("explore"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(CURIOSITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(UNEXPLORED_NEARBY_INPUT, linear)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid("Incapacitated"),
        }
    }
}

impl Default for ExploreDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for ExploreDse {
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
        // §7.3: Exploring → OpenMinded. Activity-shaped; curiosity
        // drift drops it.
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "area_explored",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn explore_dse() -> Box<dyn Dse> {
    Box::new(ExploreDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explore_dse_id_stable() {
        assert_eq!(ExploreDse::new().id().0, "explore");
    }

    #[test]
    fn explore_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            ExploreDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }
}
