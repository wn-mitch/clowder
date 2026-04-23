//! `Herbcraft::SetWard` — sibling-DSE split from the retiring cat
//! `Herbcraft` inline block.
//!
//! `CompensatedProduct` of spirituality + herbcraft_skill.
//! Eligibility: `.require("WardStrengthLow")` per §4 port (Phase
//! 4b.5). The outer `ctx.has_ward_herbs` conjunct in
//! `scoring.rs::score_actions` stays inline until a per-cat inventory
//! marker port lands `HasWardHerbs` on a future batch. The
//! ward-siege bonus at the same site remains inline — it's an inner
//! additive on a different marker (`WardsUnderSiege`), not on this
//! DSE's eligibility. Maslow tier 2.

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
            // §4 marker eligibility (Phase 4b.5): SetWard only scores
            // when colony ward strength is low. Retires the
            // `ctx.ward_strength_low` conjunct from the
            // `ward_eligible` gate at `scoring.rs:740`.
            eligibility: EligibilityFilter::new().require("WardStrengthLow"),
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
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

    #[test]
    fn herbcraft_ward_id_stable() {
        assert_eq!(HerbcraftWardDse::new().id().0, "herbcraft_ward");
    }

    #[test]
    fn herbcraft_ward_requires_ward_strength_low() {
        // Phase 4b.5: the outer `ctx.ward_strength_low` conjunct at
        // `scoring.rs:740` retires; WardStrengthLow moves onto the
        // DSE's eligibility filter.
        let dse = HerbcraftWardDse::new();
        assert_eq!(dse.eligibility().required, vec!["WardStrengthLow"]);
        assert!(dse.eligibility().forbidden.is_empty());
    }

    #[test]
    fn herbcraft_ward_rejected_without_ward_strength_low_marker() {
        // Marker absent → evaluator short-circuits to `None`, per §4's
        // "avoid computing a score that can't win" principle.
        let dse = HerbcraftWardDse::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.8_f32;
        assert!(evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch).is_none());
    }
}
