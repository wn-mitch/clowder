//! `CraftAtTanningFrame` — ticket 369, Phase 2b hide-armor crafting.
//!
//! Sibling DSE to [`CraftAtWorkshopDse`] for the 369 Phase 2b
//! `StationRequirement::TanningFrame` recipes (HideBracers,
//! HidePlatedWrap). Identical scoring shape — `WeightedSum(one,
//! diligence, playfulness)` — to keep the two craft DSEs balanced
//! against each other in the §L2.10.6 softmax pool. Per the §L2.10.10
//! sibling-DSE pattern that retired the Max-composed Herbcraft parent:
//! each station's craft is its own DSE rather than a single
//! "do-any-craft" DSE with hidden station discrimination.
//!
//! **Eligibility shape** mirrors `CraftAtWorkshopDse`:
//! - `.require(CanCraft)` — per-cat `Adult ∧ ¬Injured` capability.
//! - `.require(HasFunctionalTanningFrame)` — colony-side station
//!   presence (the 369 marker added in the same commit).
//! - `.require(HasCraftInputInInventory)` — cat carries ≥1 craft
//!   input (the same per-cat marker the Workshop DSE consults; the
//!   marker writer was extended in 369 to include the prey-byproducts
//!   Bone / Sinew / Whisker / Hide alongside the original 368 inputs).
//! - `.forbid(Incapacitated)` — standard non-Eat/Sleep/Idle gate.
//!
//! Scoring rationale identical to CraftAtWorkshopDse — see that
//! module's doc-comment.

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub struct CraftAtTanningFrameDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CraftAtTanningFrameDse {
    pub fn new() -> Self {
        Self {
            id: DseId("craft_at_tanning_frame"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    "one",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    "playfulness",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            composition: Composition::weighted_sum(vec![0.4, 0.3, 0.3]),
            eligibility: EligibilityFilter::new()
                .require(markers::CanCraft::KEY)
                .require(markers::HasFunctionalTanningFrame::KEY)
                .require(markers::HasCraftInputInInventory::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for CraftAtTanningFrameDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CraftAtTanningFrameDse {
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
                label: "crafted_at_tanning_frame",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        4
    }
}

impl crate::ai::dse::CatDse for CraftAtTanningFrameDse {
    fn action(&self) -> crate::ai::Action {
        // Shares the `Craft` action with the Workshop DSE — both
        // emit into the same DispositionKind::Crafting and use the
        // same plan-template family. The template gates the choice
        // of station via the `ZoneIs(Workshop)` vs
        // `ZoneIs(TanningFrame)` precondition on each action def.
        crate::ai::Action::Craft
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn craft_at_tanning_frame_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(CraftAtTanningFrameDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn craft_at_tanning_frame_dse_id_stable() {
        assert_eq!(
            CraftAtTanningFrameDse::new().id().0,
            "craft_at_tanning_frame"
        );
    }

    #[test]
    fn craft_at_tanning_frame_weights_sum_to_one() {
        let dse = CraftAtTanningFrameDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn craft_at_tanning_frame_eligibility_shape() {
        let dse = CraftAtTanningFrameDse::new();
        assert_eq!(
            dse.eligibility().required,
            vec![
                markers::CanCraft::KEY,
                markers::HasFunctionalTanningFrame::KEY,
                markers::HasCraftInputInInventory::KEY,
            ]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }

    #[test]
    fn craft_at_tanning_frame_is_maslow_tier_4() {
        assert_eq!(CraftAtTanningFrameDse::new().maslow_tier(), 4);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static CRAFT_AT_TANNING_FRAME_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3705,
        construct: |_| craft_at_tanning_frame_dse(),
    };
