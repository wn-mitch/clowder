//! Snake `Foraging` — active prey pursuit. Fires when hunger is
//! acute (steep logistic, midpoint 0.3 — only when very hungry)
//! and modulated by aggression personality.
//!
//! `WeightedSum` of two axes — `hunger_urgency` via `Logistic(8,
//! 0.3)` (steep, fires only under acute hunger), `aggression` via
//! `Linear(1.0, 0.0)` (aggressive snakes forage more readily).
//!
//! Maslow tier 1 — survival (feeding).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const HUNGER_URGENCY_INPUT: &str = "hunger_urgency";
pub const AGGRESSION_INPUT: &str = "aggression";

pub struct SnakeForagingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SnakeForagingDse {
    pub fn new() -> Self {
        let hunger_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.3,
        };
        let aggression_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("snake_foraging"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_URGENCY_INPUT, hunger_curve)),
                Consideration::Scalar(ScalarConsideration::new(AGGRESSION_INPUT, aggression_curve)),
            ],
            composition: Composition::weighted_sum(vec![0.7, 0.3]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for SnakeForagingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for SnakeForagingDse {
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
                label: "snake_fed_by_foraging",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn snake_foraging_dse() -> Box<dyn Dse> {
    Box::new(SnakeForagingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_foraging_id_stable() {
        assert_eq!(SnakeForagingDse::new().id().0, "snake_foraging");
    }

    #[test]
    fn snake_foraging_has_two_axes() {
        assert_eq!(SnakeForagingDse::new().considerations().len(), 2);
    }

    #[test]
    fn snake_foraging_weights_sum_to_one() {
        let sum: f32 = SnakeForagingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}
