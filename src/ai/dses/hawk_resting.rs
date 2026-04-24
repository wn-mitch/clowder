//! Hawk `Resting` — satiation- and injury-driven rest.
//!
//! `WeightedSum` of two axes — `hunger` via `Linear(1.0, 0.0)` (well-fed
//! hawks rest: higher hunger value = more sated = more likely to rest),
//! `health_fraction` via `Composite { Linear(1.0, 0.0), Invert }` (injured
//! hawks rest more: low health → high rest drive).
//!
//! Maslow tier 1 — survival (recovery).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const HUNGER_INPUT: &str = "hunger";
pub const HEALTH_FRACTION_INPUT: &str = "health_fraction";

pub struct HawkRestingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HawkRestingDse {
    pub fn new() -> Self {
        // Well-fed hawks rest: linear pass-through of hunger/satiation.
        let hunger_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // Injured hawks rest more: invert health fraction so low health
        // yields high rest drive.
        let health_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("hawk_resting"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_INPUT, hunger_curve)),
                Consideration::Scalar(ScalarConsideration::new(
                    HEALTH_FRACTION_INPUT,
                    health_curve,
                )),
            ],
            composition: Composition::weighted_sum(vec![0.5, 0.5]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for HawkRestingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HawkRestingDse {
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
                label: "hawk_rested",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn hawk_resting_dse() -> Box<dyn Dse> {
    Box::new(HawkRestingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hawk_resting_id_stable() {
        assert_eq!(HawkRestingDse::new().id().0, "hawk_resting");
    }

    #[test]
    fn hawk_resting_has_two_axes() {
        assert_eq!(HawkRestingDse::new().considerations().len(), 2);
    }

    #[test]
    fn hawk_resting_weights_sum_to_one() {
        let sum: f32 = HawkRestingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn hawk_resting_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            HawkRestingDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn hawk_resting_maslow_tier_is_one() {
        assert_eq!(HawkRestingDse::new().maslow_tier(), 1);
    }

    #[test]
    fn health_fraction_invert() {
        let dse = HawkRestingDse::new();
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Linear(1.0, 0.0) then Invert. health=0 → inner=0 → invert=1.
        // health=1 → inner=1 → invert=0.
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.0).abs() < 1e-4);
    }
}
