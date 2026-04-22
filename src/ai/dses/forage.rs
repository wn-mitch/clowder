//! `Forage` — peer of Eat in the Starvation-urgency group.
//!
//! Per §2.3 + §3.1.1: WeightedSum of `hunger_urgency + food_scarcity
//! + diligence`.
//!
//! Design intent per §3.1.1: "A starving lazy cat should still
//! forage (desperation); a diligent cat should still forage when
//! colony stores are low."

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{hangry, scarcity, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub struct ForageDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ForageDse {
    pub fn new() -> Self {
        Self {
            id: DseId("forage"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("hunger_urgency", hangry())),
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            // RtEO weights: diligence dominates — the point of Forage
            // vs. Hunt is that diligent non-bold cats choose it.
            // Hunger and scarcity still contribute, but personality
            // differentiation is what distinguishes the two
            // food-acquisition DSEs. Sum = 1.0.
            composition: Composition::weighted_sum(vec![0.3, 0.25, 0.45]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for ForageDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for ForageDse {
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
                label: "food_at_stores",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn forage_dse() -> Box<dyn Dse> {
    Box::new(ForageDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forage_dse_id_stable() {
        assert_eq!(ForageDse::new().id().0, "forage");
    }

    #[test]
    fn forage_weights_sum_to_one() {
        let dse = ForageDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn forage_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            ForageDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }
}
