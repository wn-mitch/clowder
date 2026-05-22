//! `Mentor` — Social-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1507: `WeightedSum` of 3 axes — warmth,
//! diligence, ambition. RtEO composition intentionally per the
//! design-intent note: "ambitious-but-cold cats *do* mentor (for
//! status/respect, not affection) — a real cat social dynamic."
//! CP would silence that signal.
//!
//! Eligibility: `.require(HasMentoringTarget::KEY)` (Ticket 014
//! Mentoring batch). `aspirations::update_mentoring_target_markers`
//! authors the marker per tick from the same skill-gap predicate that
//! used to live as the inline `has_mentoring_target_fn` closures in
//! `disposition.rs` / `goap.rs` (now retired).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub const WARMTH_INPUT: &str = "warmth";
pub const DILIGENCE_INPUT: &str = "diligence";
pub const AMBITION_INPUT: &str = "ambition";

pub struct MentorDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl MentorDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // 209: positive `colony_food_security` axis. Plain Logistic
        // (no Invert post-op) — output rises with food security,
        // providing positive lift when the colony is well-fed (the
        // path-1 alternative from 181's closeout). Default weight 0.0
        // ships dormant; tuning iteration lifts it.
        let lift_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        let lift_weight = scoring.mentor_food_security_weight.clamp(0.0, 1.0);
        let remainder = 1.0 - lift_weight;
        Self {
            id: DseId("mentor"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(WARMTH_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(DILIGENCE_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(AMBITION_INPUT, linear)),
                Consideration::Scalar(ScalarConsideration::new("colony_food_security", lift_curve)),
            ],
            // RtEO weights sum to 1.0. Warmth + diligence co-drive;
            // ambition is the status-seeking secondary driver. The
            // fourth axis (colony_food_security) ships at default-
            // zero weight; the other three scale by `remainder` so
            // the weight sum stays 1.0 even when balance-tuning
            // lifts the lift knob.
            composition: Composition::weighted_sum(vec![
                0.4 * remainder,
                0.4 * remainder,
                0.2 * remainder,
                lift_weight,
            ]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            // Ticket 014 Mentoring batch: also requires
            // `HasMentoringTarget` (cat sees a peer with a learnable
            // skill gap), authored by
            // `aspirations::update_mentoring_target_markers`.
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .require(markers::HasMentoringTarget::KEY),
        }
    }
}

impl Dse for MentorDse {
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
        // §7.3: Mentor is a constituent action of the Socializing
        // disposition and rides Socializing's `OpenMinded` strategy.
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "mentored_apprentice",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        // Self-actualization tier per inline (uses tier_suppression(5)
        // implicitly — actually inline at scoring.rs:722 uses tier 2
        // `tier_suppression(2)`; keep tier 2 for parity).
        2
    }
}
impl crate::ai::dse::CatDse for MentorDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Mentor
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        // Elders carry hard-won mastery and should be teaching;
        // mentee-side gate (MentorableAge, 450) excludes Stage 1/2 kittens.
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn mentor_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(MentorDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_scoring() -> ScoringConstants {
        ScoringConstants::default()
    }

    #[test]
    fn mentor_dse_id_stable() {
        assert_eq!(MentorDse::new(&default_scoring()).id().0, "mentor");
    }

    #[test]
    fn mentor_weights_sum_to_one() {
        let sum: f32 = MentorDse::new(&default_scoring())
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn mentor_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            MentorDse::new(&default_scoring()).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn mentor_food_security_tuned_to_iter1_weight() {
        // 210 iter-1: weight 0.10. The (1-w) rebalance scales the
        // existing three weights to 0.36/0.36/0.18 and the new fourth
        // axis carries 0.10, summing to 1.0.
        let scoring = default_scoring();
        assert!((scoring.mentor_food_security_weight - 0.10).abs() < 1e-4);
        let weights = MentorDse::new(&scoring).composition().weights.clone();
        assert_eq!(weights.len(), 4);
        assert!((weights[0] - 0.36).abs() < 1e-4);
        assert!((weights[1] - 0.36).abs() < 1e-4);
        assert!((weights[2] - 0.18).abs() < 1e-4);
        assert!((weights[3] - 0.10).abs() < 1e-4);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static MENTOR_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 2700,
        construct: |s| mentor_dse(s),
    };
