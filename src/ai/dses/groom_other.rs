//! `Groom(other)` — sibling-DSE split from the retiring `Max`-composed
//! cat `Groom` inline block (§L2.10.10). Allogrooming — bond-building
//! through physical contact.
//!
//! Per §2.3 rows 1026–1028 + §3.1.1 row 1484: Social-urgency peer
//! (§3.3.2 anchor = 1.0). Self-state DSE — decides *whether* the cat
//! is in a state to allogroom. The target-taking partner DSE
//! `groom_other_target_dse` (`src/ai/dses/groom_other_target.rs`)
//! decides *whom* via fondness + kinship + adjacency + warmth-need.
//!
//! 209: ethology-corrected. Real-cat allogrooming (van den Bos 1998
//! et al.) is bond + opportunity-driven; the prior `social_deficit`
//! primary axis encoded the *groomer's own* social need, which
//! double-counted what the target-taking DSE already captures via
//! `target_fondness` + `target_kinship`. Changes:
//!
//! - Add `.require(HasGroomingCandidate)` — don't fire if no nearby
//!   cat the target-taking partner could pick. Parallel to Mentor's
//!   `HasMentoringTarget` eligibility. Author:
//!   `social.rs::update_grooming_candidate_markers`.
//! - Drop `social_deficit` axis (the wrong-direction primary driver).
//! - Demote `phys_satisfaction` from `inverted_need_penalty` (hard
//!   gate: any unmet primary need zeroes the score) to a soft
//!   `Linear(0.7, 0.3)` factor with floor 0.3. Real cats also groom
//!   under tension as a defusion behavior, so well-being shouldn't
//!   hard-gate.
//! - Keep `warmth` (personality) and `social_warmth_deficit`
//!   (composite warmth + social fulfillment signal) at their
//!   existing weights.
//! - Positive food-security lift lands as the `FoodSecurityGroomLift`
//!   modifier in `src/ai/modifier.rs` (multiplicative shape `(1 + w
//!   · colony_food_security)` outside the CompensatedProduct so the
//!   gate semantics are preserved).

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
pub const SOCIAL_WARMTH_DEFICIT_INPUT: &str = "social_warmth_deficit";

pub struct GroomOtherDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl GroomOtherDse {
    pub fn new() -> Self {
        Self {
            id: DseId("groom_other"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    WARMTH_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                // 209: demoted from `inverted_need_penalty` (hard gate)
                // to a soft factor with floor 0.3. Real cats also
                // groom under tension; phys_satisfaction near zero
                // should DAMPEN allogrooming, not eliminate it.
                Consideration::Scalar(ScalarConsideration::new(
                    PHYS_SATISFACTION_INPUT,
                    Curve::Linear {
                        slope: 0.7,
                        intercept: 0.3,
                    },
                )),
                // §7.W: social_warmth fulfillment deficit. 0.1 floor
                // so groom_other isn't zeroed when social_warmth is
                // full — cats still groom for relationship/social
                // reasons.
                Consideration::Scalar(ScalarConsideration::new(
                    SOCIAL_WARMTH_DEFICIT_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.1,
                    },
                )),
            ],
            // RtM weights: warmth, phys_satisfaction, social_warmth_deficit.
            // CompensatedProduct preserves the bond-and-opportunity-gated
            // semantics that real cat allogrooming requires;
            // affiliation is handled by `groom_other_target_dse`'s
            // fondness/kinship reads (the wrong-direction primary
            // `social_deficit` axis was dropped).
            composition: Composition::compensated_product(vec![1.0, 1.0, 0.6]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            // Note: 209 originally required `HasGroomingCandidate`
            // here as a substrate gate, but `score_actions` already
            // gates `groom_other` scoring on `has_social_target`
            // (broad-phase target-existence marker), so the parallel
            // marker was redundant. The marker writer + ECS-component
            // wiring stays for trace observability + future direct
            // ScoringContext consumption.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for GroomOtherDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for GroomOtherDse {
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
        // §7.3: GroomOther is a constituent action of the Socializing
        // disposition and rides Socializing's `OpenMinded` strategy.
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState::predicate("groomed_other", |_, _| false),
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}
impl crate::ai::dse::CatDse for GroomOtherDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::GroomOther
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::ALL
    }
}

pub fn groom_other_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(GroomOtherDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groom_other_dse_id_stable() {
        assert_eq!(GroomOtherDse::new().id().0, "groom_other");
    }

    #[test]
    fn groom_other_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            GroomOtherDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn groom_other_maslow_tier_is_two() {
        assert_eq!(GroomOtherDse::new().maslow_tier(), 2);
    }

    #[test]
    fn groom_other_eligibility_only_forbids_incapacitated() {
        // 209: `score_actions` gates `groom_other` scoring on
        // `has_social_target`, so the DSE-level eligibility filter
        // only adds the §13.1 Incapacitated forbid. Adding a
        // parallel `HasGroomingCandidate::require` was redundant —
        // it would silently suppress all `groom_other` scoring
        // because the new marker isn't an authored equivalent of
        // the existing `HasSocialTarget` substrate.
        let filter = GroomOtherDse::new().eligibility().clone();
        assert!(filter.required.is_empty());
        assert!(filter.forbidden.contains(&markers::Incapacitated::KEY));
    }

    #[test]
    fn groom_other_drops_social_deficit_axis() {
        // 209 axis correction: `social_deficit` (groomer's own
        // social need) was wrong-direction per real-cat ethology.
        // Affiliation is handled by `groom_other_target_dse`. Guard
        // that the axis is gone from the self-state DSE.
        let dse = GroomOtherDse::new();
        let names: Vec<&str> = dse
            .considerations()
            .iter()
            .filter_map(|c| match c {
                Consideration::Scalar(s) => Some(s.name),
                _ => None,
            })
            .collect();
        assert!(
            !names.contains(&"social_deficit"),
            "social_deficit must not appear: got {names:?}"
        );
    }

    #[test]
    fn groom_other_phys_satisfaction_no_longer_hard_gates() {
        // 209: phys_satisfaction was demoted from
        // `inverted_need_penalty` (hard gate via low needs) to a
        // soft `Linear(0.7, 0.3)` factor. The Linear with intercept
        // 0.3 means phys_satisfaction = 0 → curve outputs 0.3
        // (not 0), so a stressed-but-bonded cat can still allogroom
        // (real-cat tension-defusion behavior).
        let dse = GroomOtherDse::new();
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
        // Sample at 0.0 — Linear(0.7, 0.3) returns 0.3 (the floor).
        let v = phys_curve.evaluate(0.0);
        assert!((v - 0.3).abs() < 1e-4, "expected 0.3 floor, got {v}");
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static GROOM_OTHER_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 700,
        construct: |_| groom_other_dse(),
    };
