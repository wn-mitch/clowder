//! `Herbcraft::PrepareRemedy` — sibling-DSE split from the retiring
//! cat `Herbcraft` inline block.
//!
//! `CompensatedProduct` of compassion + herbcraft_skill. Both gate —
//! preparing remedies requires both caring about colony injuries
//! and the skill to produce effective poultices. Eligibility:
//! `has_remedy_herbs && colony_injury_count > 0` (outer gate).
//! Maslow tier 2.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const COMPASSION_INPUT: &str = "compassion";
pub const HERBCRAFT_SKILL_INPUT: &str = "herbcraft_skill";

pub struct HerbcraftPrepareDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HerbcraftPrepareDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        Self {
            id: DseId("herbcraft_prepare"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(COMPASSION_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HERBCRAFT_SKILL_INPUT, linear)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for HerbcraftPrepareDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HerbcraftPrepareDse {
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
                label: "remedy_applied",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn herbcraft_prepare_dse() -> Box<dyn Dse> {
    Box::new(HerbcraftPrepareDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herbcraft_prepare_id_stable() {
        assert_eq!(HerbcraftPrepareDse::new().id().0, "herbcraft_prepare");
    }
}
