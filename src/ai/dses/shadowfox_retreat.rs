//! ShadowFox `Retreat` — fed-and-far-from-home den return (310 S4).
//!
//! Generalizes S2's event-driven post-ambush retreat: that immediate
//! transition stays (a landed ambush flees the scene the same tick);
//! this DSE covers the cases the event path cannot see — a fox fed on
//! prey kills wandering far from its den, or one released from any
//! state while still sated. `WeightedSum` of two axes — `satiation`
//! via `Logistic(6, 0.7)` (the same threshold band that gates the hunt)
//! and `den_distance_norm` via `Linear(1, 0)` (farther from home →
//! stronger pull).
//!
//! Outer gate in the candidate layer: no den in `ShadowFoxBeliefs` →
//! the candidate does not stand (the dispatcher returns no score).
//!
//! Maslow tier 1 — survival (self-preservation).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const SATIATION_INPUT: &str = "satiation";
/// Distance to the den normalized by the motivation scan radius,
/// clamped to [0, 1].
pub const DEN_DISTANCE_INPUT: &str = "den_distance_norm";

pub struct ShadowfoxRetreatDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ShadowfoxRetreatDse {
    pub fn new(_scoring: &ScoringConstants) -> Self {
        let considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(
                SATIATION_INPUT,
                Curve::Logistic {
                    steepness: 6.0,
                    midpoint: 0.7,
                },
            )),
            Consideration::Scalar(ScalarConsideration::new(
                DEN_DISTANCE_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )),
        ];
        let weights = vec![0.6, 0.4];

        Self {
            id: DseId("shadowfox_retreat"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for ShadowfoxRetreatDse {
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
            state: GoalState::predicate("shadowfox_at_den", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn shadowfox_retreat_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(ShadowfoxRetreatDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadowfox_retreat_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(ShadowfoxRetreatDse::new(&s).id().0, "shadowfox_retreat");
    }

    #[test]
    fn shadowfox_retreat_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = ShadowfoxRetreatDse::new(&s)
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn shadowfox_retreat_has_two_axes() {
        let s = ScoringConstants::default();
        assert_eq!(ShadowfoxRetreatDse::new(&s).considerations().len(), 2);
    }
}
