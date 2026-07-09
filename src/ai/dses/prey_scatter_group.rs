//! Prey `ScatterGroup` — herd-flush threat response (266, second prey
//! DSE).
//!
//! Where `Bolt` is an individual's evasion, `ScatterGroup` is the
//! herd's: when same-kind prey stand together and a predator commits
//! to the chase, the group explodes in divergent directions — members
//! cross paths and the pursuer's `pursue()` lead-interception loses
//! its lock (the predicted position stops predicting).
//!
//! `WeightedSum` of two axes, both live-writer substrate reads:
//!
//! | # | Axis                        | Source                                       | Curve         | Weight |
//! |---|-----------------------------|----------------------------------------------|---------------|--------|
//! | 1 | `scatter_group_affordance`  | `Affordance(ScatterGroup, me, threat)` (314) | `Linear(1,0)` | 0.55   |
//! | 2 | `threat_chase_affordance`   | `Affordance(Chase, threat, me)` (261)        | `Linear(1,0)` | 0.45   |
//!
//! Axis 1 is 314's herd heuristic (group density + threat proximity +
//! reaction readiness + health). **Eligibility trap named at the
//! candidate layer:** the writer's quartet does NOT hard-gate on the
//! group census — a lone prey still composes ≈ 0.25 × (prox + alert +
//! health) from the other three slots. The election arm in `prey_ai`
//! therefore requires ≥ 1 same-kind neighbor in sensing range before
//! this candidate stands (eligibility before scoring — the 310 S1
//! discipline; a WeightedSum cannot express the grouped-AND-committed
//! conjunction).
//!
//! Bolt and ScatterGroup stand in ONE election per (prey, threat) pair
//! (pillar 4): argmax above threshold wins; there is no second elector.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    ActivityKind, CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention,
    Termination,
};
use crate::resources::sim_constants::ScoringConstants;

pub const SCATTER_GROUP_AFFORDANCE_INPUT: &str = "scatter_group_affordance";

pub struct PreyScatterGroupDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PreyScatterGroupDse {
    pub fn new(_scoring: &ScoringConstants) -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        let considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(
                SCATTER_GROUP_AFFORDANCE_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(
                crate::ai::dses::prey_bolt::THREAT_CHASE_AFFORDANCE_INPUT,
                linear,
            )),
        ];
        let weights = vec![0.55, 0.45];

        Self {
            id: DseId("prey_scatter_group"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for PreyScatterGroupDse {
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
        // Score-only for prey (the state machine owns commitment) —
        // same convention as `prey_bolt`.
        Intention::Activity {
            kind: ActivityKind::Avoid,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn prey_scatter_group_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(PreyScatterGroupDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prey_scatter_group_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(PreyScatterGroupDse::new(&s).id().0, "prey_scatter_group");
    }

    #[test]
    fn prey_scatter_group_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = PreyScatterGroupDse::new(&s)
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn prey_scatter_group_has_two_axes() {
        let s = ScoringConstants::default();
        assert_eq!(PreyScatterGroupDse::new(&s).considerations().len(), 2);
    }
}
