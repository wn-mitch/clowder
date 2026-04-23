//! `Patrol` (cat) — Fatal-threat peer AND Territory-urgency peer
//! (§3.3.2 dual-listed). Proactive safety-seeking — the
//! above-threshold cousin of `Flee`.
//!
//! Per §2.3 + §3.1.1 row 1492: `CompensatedProduct` of 2 axes —
//! `safety_deficit` via `Logistic(6, patrol_safety_threshold)`
//! (softer than Flee's steepness=10 — Patrol is proactive, operates
//! above Flee's threshold) and `boldness` via Linear. Both gate:
//! timid cats flee instead of patrol; full-safety has nothing to
//! patrol. Maslow tier 2.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const SAFETY_DEFICIT_INPUT: &str = "safety_deficit";
pub const BOLDNESS_INPUT: &str = "boldness";

pub struct PatrolDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PatrolDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        Self {
            id: DseId("patrol"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    SAFETY_DEFICIT_INPUT,
                    Curve::Logistic {
                        steepness: 6.0,
                        midpoint: scoring.patrol_safety_threshold,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    BOLDNESS_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid("Incapacitated"),
        }
    }
}

impl Dse for PatrolDse {
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
        // §7.3: Guarding → Blind. Territory defense shouldn't flinch
        // mid-patrol. AI8 caps fixation.
        CommitmentStrategy::Blind
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "territory_patrolled",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::Blind,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn patrol_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(PatrolDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrol_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(PatrolDse::new(&s).id().0, "patrol");
    }

    #[test]
    fn patrol_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            PatrolDse::new(&s).composition().mode,
            CompositionMode::CompensatedProduct
        );
    }
}
