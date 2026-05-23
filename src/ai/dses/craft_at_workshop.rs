//! `CraftAtWorkshop` — ticket 457, Phase 2 behavioral-tool crafting.
//!
//! The elect-side DSE for the 368 Workshop substrate. Generalised
//! over every registered `StationRequirement::Workshop` recipe — the
//! DSE asks "could this cat conceivably craft anything at a Workshop
//! right now?" via the `HasCraftInputInInventory` marker, and the
//! resolver (`resolve_craft_at_workshop`) picks the specific recipe at
//! execute time. No per-recipe scoring (deferred per the ticket's
//! "future refinement: recipe variety axis"); first-light just needs
//! one craft to fire so the 368 substrate first-light gate passes.
//!
//! **Eligibility shape** mirrors `SmokeMeatDse`:
//! - `.require(CanCraft)` — per-cat `Adult ∧ ¬Injured` capability.
//! - `.require(HasFunctionalWorkshop)` — colony-side station presence.
//! - `.require(HasCraftInputInInventory)` — cat carries ≥1 Phase 2
//!   recipe input. Keeps the DSE silent on cats with empty pockets so
//!   the planner doesn't form a trip-to-Workshop-then-fail loop.
//! - `.forbid(Incapacitated)` — standard non-Eat/Sleep/Idle gate.
//!
//! **Scoring** — three scalar considerations, all Linear, summing to
//! 1.0 in WeightedSum composition:
//! - `one` (weight 0.4) — always-on base rate. "Crafting is a good
//!   way to spend a few ticks when you have the inputs."
//! - `diligence` (weight 0.3) — workshop work scales with the cat's
//!   do-the-work personality axis.
//! - `playfulness` (weight 0.3) — Phase 2 outputs are behavioral
//!   tools (grooming brush, play bundle, courtship gift) that
//!   enhance social/play outcomes. Playful cats lean into the craft.
//!
//! No spatial axis. With typically one Workshop on the seed-42 map,
//! proximity weighting adds plumbing (NearestWorkshop anchor +
//! `CatAnchorPositions` field + threading through ~10 sites) for
//! marginal first-light benefit. The follow-on opens the spatial axis
//! once first-light confirms cats are crafting at all.

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub struct CraftAtWorkshopDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CraftAtWorkshopDse {
    pub fn new() -> Self {
        Self {
            id: DseId("craft_at_workshop"),
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
                .require(markers::HasFunctionalWorkshop::KEY)
                .require(markers::HasCraftInputInInventory::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for CraftAtWorkshopDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CraftAtWorkshopDse {
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
                label: "crafted_at_workshop",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        4
    }
}

impl crate::ai::dse::CatDse for CraftAtWorkshopDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Craft
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn craft_at_workshop_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(CraftAtWorkshopDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn craft_at_workshop_dse_id_stable() {
        assert_eq!(CraftAtWorkshopDse::new().id().0, "craft_at_workshop");
    }

    #[test]
    fn craft_at_workshop_weights_sum_to_one() {
        let dse = CraftAtWorkshopDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn craft_at_workshop_eligibility_shape() {
        let dse = CraftAtWorkshopDse::new();
        assert_eq!(
            dse.eligibility().required,
            vec![
                markers::CanCraft::KEY,
                markers::HasFunctionalWorkshop::KEY,
                markers::HasCraftInputInInventory::KEY,
            ]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }

    #[test]
    fn craft_at_workshop_is_maslow_tier_4() {
        assert_eq!(CraftAtWorkshopDse::new().maslow_tier(), 4);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static CRAFT_AT_WORKSHOP_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3700,
        construct: |_| craft_at_workshop_dse(),
    };
