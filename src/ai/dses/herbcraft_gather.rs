//! `Herbcraft::GatherHerbs` — sibling-DSE split from the retiring
//! cat `Herbcraft` inline block (§L2.10.10).
//!
//! `CompensatedProduct` of spirituality + herbcraft_skill. Both gate
//! — collecting herbs is a devotional/craft activity; neither pure
//! spirituality nor pure skill suffices alone. Eligibility:
//! `has_herbs_nearby` (outer gate). Maslow tier 2.
//!
//! Emergency bonuses (ward-corruption-triggered gather boost) stay
//! in the inline scorer until the §3.5 modifier pipeline lands in
//! Phase 3d.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const SPIRITUALITY_INPUT: &str = "spirituality";
pub const HERBCRAFT_SKILL_INPUT: &str = "herbcraft_skill";

pub struct HerbcraftGatherDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HerbcraftGatherDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        Self {
            id: DseId("herbcraft_gather"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SPIRITUALITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HERBCRAFT_SKILL_INPUT, linear)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for HerbcraftGatherDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HerbcraftGatherDse {
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
                label: "herbs_in_inventory",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn herbcraft_gather_dse() -> Box<dyn Dse> {
    Box::new(HerbcraftGatherDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herbcraft_gather_id_stable() {
        assert_eq!(HerbcraftGatherDse::new().id().0, "herbcraft_gather");
    }
}
