//! Hawk `Hunting` — hunger-driven prey pursuit.
//!
//! `WeightedSum` of two axes — `hunger_urgency` via `Logistic(6, 0.5)`
//! (sigmoid ramp centered at half-hungry), `prey_nearby` via
//! `Linear(1.0, 0.0)` (proportional to visible prey density).
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
pub const PREY_NEARBY_INPUT: &str = "prey_nearby";

pub struct HawkHuntingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HawkHuntingDse {
    pub fn new() -> Self {
        let hunger_curve = Curve::Logistic {
            steepness: 6.0,
            midpoint: 0.5,
        };
        let prey_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("hawk_hunting"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_URGENCY_INPUT, hunger_curve)),
                Consideration::Scalar(ScalarConsideration::new(PREY_NEARBY_INPUT, prey_curve)),
            ],
            composition: Composition::weighted_sum(vec![0.7, 0.3]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for HawkHuntingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HawkHuntingDse {
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
                label: "hawk_fed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn hawk_hunting_dse() -> Box<dyn Dse> {
    Box::new(HawkHuntingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hawk_hunting_id_stable() {
        assert_eq!(HawkHuntingDse::new().id().0, "hawk_hunting");
    }

    #[test]
    fn hawk_hunting_has_two_axes() {
        assert_eq!(HawkHuntingDse::new().considerations().len(), 2);
    }

    #[test]
    fn hawk_hunting_weights_sum_to_one() {
        let sum: f32 = HawkHuntingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn hawk_hunting_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            HawkHuntingDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn hawk_hunting_maslow_tier_is_one() {
        assert_eq!(HawkHuntingDse::new().maslow_tier(), 1);
    }
}
