//! ShadowFox `Patrol` — scored nocturnal default (310 S4).
//!
//! The corruption-born predator's idle disposition, day-phase weighted:
//! `night_scalar` through `Linear(0.08, 0.02)` gives 0.02 by day (below
//! the motivation pressure floor — daytime patrol does not stand for
//! election, so the fox holds whatever state it is in) and 0.10 at
//! night (a weak candidate that wins only quiet elections). Re-electing
//! Patrol while already Patrolling is a no-op (`same_motivation_kind`),
//! so the patrol jitter's wandering is not reset each cadence.
//!
//! Maslow tier 1.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const NIGHT_SCALAR_INPUT: &str = "night_scalar";

pub struct ShadowfoxPatrolDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ShadowfoxPatrolDse {
    pub fn new(_scoring: &ScoringConstants) -> Self {
        let considerations = vec![Consideration::Scalar(ScalarConsideration::new(
            NIGHT_SCALAR_INPUT,
            Curve::Linear {
                slope: 0.08,
                intercept: 0.02,
            },
        ))];
        let weights = vec![1.0];

        Self {
            id: DseId("shadowfox_patrol"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for ShadowfoxPatrolDse {
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
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState::predicate("shadowfox_patrolling", |_, _| false),
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn shadowfox_patrol_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(ShadowfoxPatrolDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadowfox_patrol_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(ShadowfoxPatrolDse::new(&s).id().0, "shadowfox_patrol");
    }

    #[test]
    fn shadowfox_patrol_single_axis() {
        let s = ScoringConstants::default();
        assert_eq!(ShadowfoxPatrolDse::new(&s).considerations().len(), 1);
    }
}
