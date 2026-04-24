//! Snake `Fleeing` — retreat from threats. Snakes are timid; even
//! a single nearby cat saturates the threat signal.
//!
//! `WeightedSum` of two axes — `health_deficit` via `Logistic(8,
//! 0.5)` (injury-panic threshold), `cats_nearby` via `Linear(1.0,
//! 0.0)` (saturates at 1 since input is 0-1 from the scalar map —
//! one cat is enough to provoke flight).
//!
//! Maslow tier 1 — survival (escape).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";
pub const CATS_NEARBY_INPUT: &str = "cats_nearby";

pub struct SnakeFleeingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SnakeFleeingDse {
    pub fn new() -> Self {
        let health_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        // Input is already 0-1 from the scalar map; Linear(1,0)
        // passes it through directly.
        let cats_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("snake_fleeing"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, health_curve)),
                Consideration::Scalar(ScalarConsideration::new(CATS_NEARBY_INPUT, cats_curve)),
            ],
            composition: Composition::weighted_sum(vec![0.5, 0.5]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for SnakeFleeingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for SnakeFleeingDse {
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
                label: "snake_fled_to_safety",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn snake_fleeing_dse() -> Box<dyn Dse> {
    Box::new(SnakeFleeingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_fleeing_id_stable() {
        assert_eq!(SnakeFleeingDse::new().id().0, "snake_fleeing");
    }

    #[test]
    fn snake_fleeing_has_two_axes() {
        assert_eq!(SnakeFleeingDse::new().considerations().len(), 2);
    }

    #[test]
    fn snake_fleeing_weights_sum_to_one() {
        let sum: f32 = SnakeFleeingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}
