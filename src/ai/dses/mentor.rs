//! `Mentor` — Social-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1507: `WeightedSum` of 3 axes — warmth,
//! diligence, ambition. RtEO composition intentionally per the
//! design-intent note: "ambitious-but-cold cats *do* mentor (for
//! status/respect, not affection) — a real cat social dynamic."
//! CP would silence that signal.
//!
//! Eligibility: `has_mentoring_target` (outer gate until §4 port).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const WARMTH_INPUT: &str = "warmth";
pub const DILIGENCE_INPUT: &str = "diligence";
pub const AMBITION_INPUT: &str = "ambition";

pub struct MentorDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl MentorDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        Self {
            id: DseId("mentor"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(WARMTH_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(DILIGENCE_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(AMBITION_INPUT, linear)),
            ],
            // RtEO weights sum to 1.0. Warmth + diligence co-drive;
            // ambition is the status-seeking secondary driver.
            composition: Composition::weighted_sum(vec![0.4, 0.4, 0.2]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for MentorDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for MentorDse {
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
                label: "mentored_apprentice",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        // Self-actualization tier per inline (uses level_suppression(5)
        // implicitly — actually inline at scoring.rs:722 uses tier 2
        // `level_suppression(2)`; keep tier 2 for parity).
        2
    }
}

pub fn mentor_dse() -> Box<dyn Dse> {
    Box::new(MentorDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentor_dse_id_stable() {
        assert_eq!(MentorDse::new().id().0, "mentor");
    }

    #[test]
    fn mentor_weights_sum_to_one() {
        let sum: f32 = MentorDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn mentor_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            MentorDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }
}
