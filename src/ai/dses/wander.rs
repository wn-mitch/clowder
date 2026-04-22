//! `Wander` — Exploration-urgency peer (§3.3.2 anchor = 1.0). The
//! spec's canonical WS example (§3.1 summary), because its base_rate
//! axis exemplifies the "keep available at zero drive" RtEO pattern.
//!
//! Per §2.3 + §3.1.1 row 1502: `WeightedSum` of 3 axes — curiosity
//! (Linear), base_rate (Linear with intercept = wander_base),
//! playfulness (Linear, additive bonus). §3.3.2 row note: "Wander
//! caps below Explore" (Wander is a base-rate fallback when nothing
//! unexplored is nearby).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    ActivityKind, CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention,
    Termination,
};
use crate::resources::sim_constants::ScoringConstants;

pub const CURIOSITY_INPUT: &str = "curiosity";
pub const ONE_INPUT: &str = "one";
pub const PLAYFULNESS_INPUT: &str = "playfulness";

pub struct WanderDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl WanderDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let base_curve = Curve::Linear {
            slope: 0.0,
            intercept: scoring.wander_base,
        };
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        Self {
            id: DseId("wander"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(CURIOSITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(ONE_INPUT, base_curve)),
                Consideration::Scalar(ScalarConsideration::new(PLAYFULNESS_INPUT, linear)),
            ],
            // RtEO sum = 1.0. Curiosity dominates; base_rate keeps
            // Wander available at zero curiosity; playfulness rider.
            composition: Composition::weighted_sum(vec![0.5, 0.2, 0.3]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for WanderDse {
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
        Intention::Activity {
            kind: ActivityKind::Wander,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn wander_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(WanderDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wander_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(WanderDse::new(&s).id().0, "wander");
    }

    #[test]
    fn wander_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = WanderDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn wander_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            WanderDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }
}
