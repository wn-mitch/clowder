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
//! `require(HasHandoffRecipient)`. The colony-scoped marker is
//! authored by `update_colony_building_markers` (ticket 188 wave-
//! closeout) from the existence of any living kitten — adults hand to
//! kittens, so the DSE is dormant when the colony has no kittens.

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
                .require(markers::HasHandoffRecipient::KEY),
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

pub fn handing_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
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
        // gates the DSE dormant when no kitten exists in the colony,
        // so this curve only fires when the substrate has a recipient.
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
    fn handing_eligibility_requires_handoff_recipient() {
        let dse = HandingDse::new(&defaults());
        let elig = dse.eligibility();
        assert!(
            elig.required.contains(&markers::HasHandoffRecipient::KEY),
            "Handing must require HasHandoffRecipient",
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
