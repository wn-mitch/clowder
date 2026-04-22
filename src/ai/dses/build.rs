//! `Build` — Work-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1493: `WeightedSum` of 3 axes — diligence
//! (Linear), site_presence (Piecewise `(0, 0), (1, build_site_bonus)`),
//! repair_presence (Piecewise `(0, 0), (1, build_repair_bonus)`).
//! RtEO: site presence drives even low-diligence cats ("there's
//! literally a half-built wall here"); repair need drives build
//! independently.
//!
//! Maslow tier 2 — Build is a safety-infrastructure action that
//! shouldn't be gated on pre-existing safety (chicken-and-egg per
//! the old inline comment), but a hungry cat still shouldn't build.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{piecewise, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const DILIGENCE_INPUT: &str = "diligence";
pub const SITE_PRESENCE_INPUT: &str = "has_construction_site";
pub const REPAIR_PRESENCE_INPUT: &str = "has_damaged_building";

pub struct BuildDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl BuildDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        Self {
            id: DseId("build"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    DILIGENCE_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    SITE_PRESENCE_INPUT,
                    piecewise(vec![(0.0, 0.0), (1.0, scoring.build_site_bonus)]),
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    REPAIR_PRESENCE_INPUT,
                    piecewise(vec![(0.0, 0.0), (1.0, scoring.build_repair_bonus)]),
                )),
            ],
            // RtEO sum = 1.0. Diligence is primary; site + repair are
            // binary presence bonuses.
            composition: Composition::weighted_sum(vec![0.5, 0.25, 0.25]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for BuildDse {
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
                label: "built_or_repaired",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn build_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(BuildDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(BuildDse::new(&s).id().0, "build");
    }

    #[test]
    fn build_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = BuildDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}
