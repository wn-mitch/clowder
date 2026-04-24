//! Hawk `Fleeing` — injury-driven escape response.
//!
//! `WeightedSum` of two axes — `health_deficit` via `Logistic(8, 0.5)`
//! (injury-panic threshold), `boldness` via `Composite { Linear(slope=
//! 0.5), Invert }` (damped invert — timid hawks flee more).
//!
//! Maslow tier 1 — survival (threat response).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";
pub const BOLDNESS_INPUT: &str = "boldness";

pub struct HawkFleeingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HawkFleeingDse {
    pub fn new() -> Self {
        let health_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        // Damped invert: Linear(slope=0.5) maps boldness=1.0 → 0.5,
        // then Invert gives (1 - 0.5) = 0.5. Max-bold hawk still
        // contributes 0.5; timid hawk (bold=0) contributes 1.0.
        let boldness_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 0.5,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("hawk_fleeing"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, health_curve)),
                Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_curve)),
            ],
            composition: Composition::weighted_sum(vec![0.65, 0.35]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for HawkFleeingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HawkFleeingDse {
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
                label: "hawk_fled_to_safety",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn hawk_fleeing_dse() -> Box<dyn Dse> {
    Box::new(HawkFleeingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hawk_fleeing_id_stable() {
        assert_eq!(HawkFleeingDse::new().id().0, "hawk_fleeing");
    }

    #[test]
    fn hawk_fleeing_has_two_axes() {
        assert_eq!(HawkFleeingDse::new().considerations().len(), 2);
    }

    #[test]
    fn hawk_fleeing_weights_sum_to_one() {
        let sum: f32 = HawkFleeingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn hawk_fleeing_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            HawkFleeingDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn hawk_fleeing_maslow_tier_is_one() {
        assert_eq!(HawkFleeingDse::new().maslow_tier(), 1);
    }

    #[test]
    fn boldness_damped_invert() {
        let dse = HawkFleeingDse::new();
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Linear(slope=0.5) then Invert. boldness=0 → inner=0 → invert=1.
        // boldness=1 → inner=0.5 → invert=0.5.
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.5).abs() < 1e-4);
    }
}
