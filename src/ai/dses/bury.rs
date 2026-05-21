//! Ticket 035 — `Bury` self-state DSE. Pairs with
//! [`bury_target_dse`](super::bury_target::bury_target_dse) which
//! decides *which* corpse via proximity + bond + kinship + cooldown.
//!
//! The self-state DSE answers *whether* the cat is in a state to bury
//! a colony-mate right now. Eligibility-gated by `HasUnburiedCorpse`
//! (sensing-pre-pass author per §4.7), so cats with no unburied
//! corpse in `burial_sense_range` never score. Forbidden when
//! `Incapacitated`.
//!
//! Composition: `[warmth, phys_satisfaction (soft floor 0.3)]` ×
//! `compensated_product`. Mirrors `groom_other_dse`'s shape minus the
//! `social_warmth_deficit` axis (no direct fulfillment analog for
//! burial; the affective pull is "caring for the dead is a
//! community-belonging act," fully encoded in `personality.warmth`).
//! `phys_satisfaction` is a soft factor, not a hard gate — a cat
//! under tension can still witness a death and act on it.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const WARMTH_INPUT: &str = "warmth";
pub const PHYS_SATISFACTION_INPUT: &str = "phys_satisfaction";

pub struct BuryDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl BuryDse {
    pub fn new() -> Self {
        Self {
            id: DseId("bury"),
            considerations: vec![
                // Personality — warmth-toward-others is the felt signal
                // for "caring for the dead is a community-belonging
                // act." Mirrors GroomOther's primary axis.
                Consideration::Scalar(ScalarConsideration::new(
                    WARMTH_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                // Soft factor with 0.3 floor — a cat under physiological
                // tension can still react to death (real-cat allogrooming
                // ethology generalizes here: tension does not eliminate
                // bond-affirming behavior, it dampens it).
                Consideration::Scalar(ScalarConsideration::new(
                    PHYS_SATISFACTION_INPUT,
                    Curve::Linear {
                        slope: 0.7,
                        intercept: 0.3,
                    },
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            // §13.1 + 035: gate on HasUnburiedCorpse (sensing pre-pass
            // marker), forbid Incapacitated.
            eligibility: EligibilityFilter::new()
                .require(markers::HasUnburiedCorpse::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for BuryDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for BuryDse {
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
        // 035: Burying is Pattern B (single-interaction). SingleMinded
        // matches Mentoring / Mating — once committed, follow through
        // until the corpse is buried or the plan hard-fails.
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "burial_performed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        // 035: tier 3 (Belonging).
        3
    }
}
impl crate::ai::dse::CatDse for BuryDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Bury
    }

    fn always_emit_zero(&self) -> bool {
        true
    }
}


pub fn bury_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(BuryDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bury_dse_id_stable() {
        assert_eq!(BuryDse::new().id().0, "bury");
    }

    #[test]
    fn bury_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            BuryDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn bury_maslow_tier_is_three() {
        assert_eq!(BuryDse::new().maslow_tier(), 3);
    }

    #[test]
    fn bury_eligibility_requires_unburied_corpse_forbids_incapacitated() {
        let filter = BuryDse::new().eligibility().clone();
        assert!(filter.required.contains(&markers::HasUnburiedCorpse::KEY));
        assert!(filter.forbidden.contains(&markers::Incapacitated::KEY));
    }

    #[test]
    fn bury_has_two_axes() {
        assert_eq!(BuryDse::new().considerations().len(), 2);
    }

    #[test]
    fn bury_phys_satisfaction_is_soft_floor() {
        // 035: Linear(0.7, 0.3) means phys_satisfaction = 0 → curve
        // outputs 0.3 (the floor), so a stressed cat can still witness
        // and bury a death.
        let dse = BuryDse::new();
        let phys_curve = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(s) if s.name == PHYS_SATISFACTION_INPUT => {
                    Some(s.curve.clone())
                }
                _ => None,
            })
            .expect("phys_satisfaction axis present");
        let v = phys_curve.evaluate(0.0);
        assert!((v - 0.3).abs() < 1e-4, "expected 0.3 floor, got {v}");
    }

    #[test]
    fn bury_default_strategy_is_single_minded() {
        // 035: Pattern B — once a cat commits to burying, follow
        // through. Mirrors Mentoring / Mating.
        assert_eq!(
            BuryDse::new().default_strategy(),
            CommitmentStrategy::SingleMinded
        );
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static BURY_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 800,
        construct: |_| bury_dse(),
    };
