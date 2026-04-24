//! `Explore` — Exploration-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1501: `CompensatedProduct` of 2 axes —
//! curiosity (Linear, scaled by `explore_curiosity_scale`) +
//! unexplored_nearby (Logistic saturation — sharp decay past 70%
//! explored). Both gate.
//!
//! Ticket 001 Sub-2: the original identity curves kept both axes
//! perpetually near 1.0, making Explore only Maslow-modulated.
//! The Logistic curve on `unexplored_nearby` creates a threshold
//! decay; the curiosity scale (0.7) differentiates low- vs
//! high-curiosity cats.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{explore_saturation, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub const CURIOSITY_INPUT: &str = "curiosity";
pub const UNEXPLORED_NEARBY_INPUT: &str = "unexplored_nearby";

pub struct ExploreDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ExploreDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        Self {
            id: DseId("explore"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    CURIOSITY_INPUT,
                    Curve::Linear {
                        slope: scoring.explore_curiosity_scale,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    UNEXPLORED_NEARBY_INPUT,
                    explore_saturation(),
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for ExploreDse {
    fn default() -> Self {
        Self::new(&ScoringConstants::default())
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

pub fn explore_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(ExploreDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_scoring() -> ScoringConstants {
        ScoringConstants::default()
    }

    #[test]
    fn explore_dse_id_stable() {
        assert_eq!(ExploreDse::new(&default_scoring()).id().0, "explore");
    }

    #[test]
    fn explore_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            ExploreDse::new(&default_scoring()).composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn explore_curiosity_uses_scale() {
        let sc = default_scoring();
        let dse = ExploreDse::new(&sc);
        // The curiosity consideration should use explore_curiosity_scale
        // (0.7), not the identity curve. At input=1.0, output = 0.7.
        if let Consideration::Scalar(ref c) = dse.considerations()[0] {
            let output = c.curve.evaluate(1.0);
            assert!(
                (output - sc.explore_curiosity_scale).abs() < 1e-5,
                "curiosity curve at 1.0 should be {}, got {output}",
                sc.explore_curiosity_scale,
            );
        } else {
            panic!("first consideration should be Scalar");
        }
    }

    #[test]
    fn explore_saturation_suppresses_explored_area() {
        let dse = ExploreDse::new(&default_scoring());
        // When unexplored_nearby = 0.1 (90% explored), the Logistic
        // should output < 0.15 — effectively suppressing Explore.
        if let Consideration::Scalar(ref c) = dse.considerations()[1] {
            let output = c.curve.evaluate(0.1);
            assert!(
                output < 0.15,
                "unexplored_nearby=0.1 should suppress; got {output}"
            );
        } else {
            panic!("second consideration should be Scalar");
        }
    }

    #[test]
    fn explore_score_meaningful_when_fresh() {
        let dse = ExploreDse::new(&default_scoring());
        // When unexplored_nearby = 1.0 (fresh area), the saturation
        // curve should be near 1.0.
        if let Consideration::Scalar(ref c) = dse.considerations()[1] {
            let output = c.curve.evaluate(1.0);
            assert!(
                output > 0.99,
                "unexplored_nearby=1.0 should be near 1.0; got {output}"
            );
        } else {
            panic!("second consideration should be Scalar");
        }
    }
}
