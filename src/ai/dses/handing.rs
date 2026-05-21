//! 176 / 188 `Handing` DSE — hand surplus food to a kitten recipient.
//! Sibling to Discarding (drop on ground) and Trashing (carry to
//! Midden); shares the `inventory_excess` axis so an adult with a
//! food-stuffed inventory has parallel disposal options that depend
//! on which colony substrate is available.
//!
//! **Composition.** Single `inventory_excess` axis through a Logistic
//! curve (slope/midpoint sourced from
//! `ScoringConstants::disposal_inventory_excess_*`). Per memory
//! feedback "single-axis perception scalars": colony state composes
//! at the eligibility-filter layer, not by folding into the scalar.
//! The recipient identity itself is resolved at dispatch time
//! (`goap.rs::HandoffItem` falls back to the nearest hungry kitten).
//!
//! **Eligibility.** `forbid(Incapacitated)` AND
//! `require(HasDependentCat)`. The colony-scoped marker (renamed from
//! `HasHandoffRecipient` in ticket 410) is authored by
//! `update_colony_building_markers` from the existence of any care
//! dependent — currently any living kitten. Adults hand to dependents,
//! so the DSE is dormant when the colony has no one needing care.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub struct HandingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HandingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        Self {
            id: DseId("handoff"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                "inventory_excess",
                Curve::Logistic {
                    steepness: scoring.disposal_inventory_excess_slope,
                    midpoint: scoring.disposal_inventory_excess_midpoint,
                },
            ))],
            composition: Composition::weighted_sum(vec![1.0]),
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .require(markers::HasDependentCat::KEY),
        }
    }
}

impl Dse for HandingDse {
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
                label: "handed_off_surplus",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}
impl crate::ai::dse::CatDse for HandingDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Handoff
    }
}


pub fn handing_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(HandingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ScoringConstants {
        ScoringConstants::default()
    }

    #[test]
    fn handing_dse_id_stable() {
        assert_eq!(HandingDse::new(&defaults()).id().0, "handoff");
    }

    #[test]
    fn handing_curve_lifts_with_inventory_excess() {
        // 188: replaced 178's default-zero curve with the same Logistic
        // shape Discarding/Trashing use on `inventory_excess`. Empty
        // inventory → near-zero score; full → near-one. Eligibility
        // gates the DSE dormant when no care dependent exists in the
        // colony, so this curve only fires when the substrate has a
        // recipient (via HasDependentCat).
        let dse = HandingDse::new(&defaults());
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!(c.evaluate(0.0) < 0.05, "empty inventory → near-zero score");
        assert!((c.evaluate(0.5) - 0.5).abs() < 1e-3, "midpoint → 0.5");
        assert!(c.evaluate(1.0) > 0.95, "full inventory → near-one score");
    }

    #[test]
    fn handing_eligibility_requires_dependent_cat() {
        let dse = HandingDse::new(&defaults());
        let elig = dse.eligibility();
        assert!(
            elig.required.contains(&markers::HasDependentCat::KEY),
            "Handing must require HasDependentCat",
        );
        assert!(
            elig.forbidden.contains(&markers::Incapacitated::KEY),
            "Handing must forbid Incapacitated",
        );
    }

    #[test]
    fn handing_maslow_tier_is_one() {
        assert_eq!(HandingDse::new(&defaults()).maslow_tier(), 1);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static HANDING_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3600,
        construct: |s| handing_dse(s),
    };
