//! Snake `Ambushing` — sit-and-wait predation strategy. Snakes
//! coil near prey trails and strike when hungry enough and patience
//! is high.
//!
//! `WeightedSum` of two axes — `hunger_urgency` via `Logistic(5,
//! 0.5)` (moderate ramp centered at half-hunger), `patience` via
//! `Linear(1.0, 0.0)` (personality modulator — patient snakes
//! prefer ambush over active foraging).
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
pub const PATIENCE_INPUT: &str = "patience";

pub struct SnakeAmbushingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SnakeAmbushingDse {
    pub fn new() -> Self {
        let hunger_curve = Curve::Logistic {
            steepness: 5.0,
            midpoint: 0.5,
        };
        let patience_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("snake_ambushing"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_URGENCY_INPUT, hunger_curve)),
                Consideration::Scalar(ScalarConsideration::new(PATIENCE_INPUT, patience_curve)),
            ],
            composition: Composition::weighted_sum(vec![0.6, 0.4]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for SnakeAmbushingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for SnakeAmbushingDse {
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
                label: "snake_fed_by_ambush",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn snake_ambushing_dse() -> Box<dyn Dse> {
    Box::new(SnakeAmbushingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_ambushing_id_stable() {
        assert_eq!(SnakeAmbushingDse::new().id().0, "snake_ambushing");
    }

    #[test]
    fn snake_ambushing_has_two_axes() {
        assert_eq!(SnakeAmbushingDse::new().considerations().len(), 2);
    }

    #[test]
    fn snake_ambushing_weights_sum_to_one() {
        let sum: f32 = SnakeAmbushingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn snake_ambushing_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            SnakeAmbushingDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }
}
