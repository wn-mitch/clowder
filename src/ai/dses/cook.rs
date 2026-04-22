//! `Cook` — peer of Eat/Hunt/Forage in the Starvation-urgency group.
//!
//! Per §3.1.1: "Base rate and scarcity urgency trade off — cooking
//! is ongoing activity plus scarcity response, not strictly gated on
//! either." WeightedSum of `base_rate + food_scarcity + diligence`.
//!
//! Maslow tier 2 — comment at `scoring.rs:738` names Cook as a
//! food-buffer multiplier analogous to Farm, "Level 2 suppression
//! (phys only)."
//!
//! **Cook-specific eligibility** — today gated at the outer level on
//! `has_functional_kitchen && has_raw_food_in_stores && hunger >
//! cook_hunger_gate`. The §4-driven port turns those into
//! `HasFunctionalKitchen` + `HasRawFoodInStores` markers plus an
//! implicit "not too starving" filter. Phase 3c.1a keeps the outer
//! gate in `score_actions`; Phase 3d flips to marker filtering.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{scarcity, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub struct CookDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CookDse {
    pub fn new() -> Self {
        Self {
            id: DseId("cook"),
            considerations: vec![
                // base_rate: constant "cooking is always worth
                // something when eligible." Input is a dummy "one"
                // (always 1.0); the Linear slope carries the magnitude.
                Consideration::Scalar(ScalarConsideration::new(
                    "one",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            // RtEO sum = 1.0. Base rate carries the "always worth
            // doing" floor; scarcity escalates under colony stress;
            // diligence is the personality weight.
            composition: Composition::weighted_sum(vec![0.4, 0.3, 0.3]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for CookDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CookDse {
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
                label: "food_cooked_at_kitchen",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn cook_dse() -> Box<dyn Dse> {
    Box::new(CookDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cook_dse_id_stable() {
        assert_eq!(CookDse::new().id().0, "cook");
    }

    #[test]
    fn cook_weights_sum_to_one() {
        let dse = CookDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn cook_is_maslow_tier_2() {
        assert_eq!(CookDse::new().maslow_tier(), 2);
    }
}
