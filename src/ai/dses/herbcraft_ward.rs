//! `Herbcraft::SetWard` — sibling-DSE split from the retiring cat
//! `Herbcraft` inline block.
//!
//! `CompensatedProduct` of spirituality + herbcraft_skill.
//! Eligibility: `ward_strength_low && (has_ward_herbs || (corruption
//! && thornbriar_available))` (outer gate). The
//! corruption-emergency and ward-siege bonuses stay inline until the
//! §3.5 modifier pipeline lands. Maslow tier 2.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const SPIRITUALITY_INPUT: &str = "spirituality";
pub const HERBCRAFT_SKILL_INPUT: &str = "herbcraft_skill";

pub struct HerbcraftWardDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HerbcraftWardDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        Self {
            id: DseId("herbcraft_ward"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SPIRITUALITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HERBCRAFT_SKILL_INPUT, linear)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for HerbcraftWardDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HerbcraftWardDse {
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
                label: "ward_placed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn herbcraft_ward_dse() -> Box<dyn Dse> {
    Box::new(HerbcraftWardDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herbcraft_ward_id_stable() {
        assert_eq!(HerbcraftWardDse::new().id().0, "herbcraft_ward");
    }
}
