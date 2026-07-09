//! ShadowFox `Hunt` — deliberate, hunger-driven cat predation (310 S4).
//!
//! Replaces the legacy 5%/tick stalk roll as the hunt entry (pillar 2:
//! this DSE is the substrate lever; the roll retires in the same
//! commit). `WeightedSum` of three axes — `hunger_urgency`
//! (1 − satiation) via `Logistic(6, 0.7)` (pressing only once satiation
//! has decayed well down, matching S1's eligibility zone),
//! `cat_in_scan` via `Linear(1, 0)` (a target must exist), and
//! `night_scalar` via `Linear(1, 0)` (corruption-born predators hunt in
//! the dark — the day_phase_scalar precedent).
//!
//! 265's shadowfox affordance slice lands here as the conditional
//! `best_cat_ambush_affordance` axis (first-light 0.10): max
//! `Affordance(Ambush, fox, cat)` over cats in scan, fed by the
//! concealment-keyed estimator in `write_wildlife_vs_cat` (this commit
//! replaces the 0.0 placeholder row).
//!
//! Outer gates live in the dispatcher/candidate layer (S1 discipline —
//! eligibility before scoring): satiation ≥ the stalk threshold removes
//! the candidate entirely (a fed predator does not stand for the hunt
//! election), and the score must clear the motivation pressure floor.
//!
//! Maslow tier 1 — survival (feeding).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const HUNGER_URGENCY_INPUT: &str = "hunger_urgency";
pub const CAT_IN_SCAN_INPUT: &str = "cat_in_scan";
pub const NIGHT_SCALAR_INPUT: &str = "night_scalar";
/// Max `Affordance(Ambush, fox, cat)` over cats in the motivation scan.
pub const CAT_AMBUSH_AFFORDANCE_INPUT: &str = "best_cat_ambush_affordance";

pub struct ShadowfoxHuntDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ShadowfoxHuntDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let mut considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(
                HUNGER_URGENCY_INPUT,
                Curve::Logistic {
                    steepness: 6.0,
                    midpoint: 0.7,
                },
            )),
            Consideration::Scalar(ScalarConsideration::new(
                CAT_IN_SCAN_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )),
            Consideration::Scalar(ScalarConsideration::new(
                NIGHT_SCALAR_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )),
        ];
        let mut weights = vec![0.6, 0.25, 0.15];

        // 265 slice — conditional Ambush-affordance axis (the 264
        // socialize_target shape). Base three scale by `(1 − extra)`
        // so the WeightedSum stays at 1.0.
        let affordance_w = scoring
            .shadowfox_hunt_cat_ambush_affordance_weight
            .clamp(0.0, 1.0);
        if affordance_w > 0.0 {
            let scale = 1.0 - affordance_w;
            for w in &mut weights {
                *w *= scale;
            }
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                CAT_AMBUSH_AFFORDANCE_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(affordance_w);
        }

        Self {
            id: DseId("shadowfox_hunt"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for ShadowfoxHuntDse {
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
            state: GoalState::predicate("shadowfox_fed", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn shadowfox_hunt_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(ShadowfoxHuntDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadowfox_hunt_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(ShadowfoxHuntDse::new(&s).id().0, "shadowfox_hunt");
    }

    #[test]
    fn shadowfox_hunt_has_three_axes_when_zeroed() {
        let mut s = ScoringConstants::default();
        s.shadowfox_hunt_cat_ambush_affordance_weight = 0.0;
        assert_eq!(ShadowfoxHuntDse::new(&s).considerations().len(), 3);
    }

    #[test]
    fn shadowfox_hunt_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = ShadowfoxHuntDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn ambush_affordance_axis_active_at_default() {
        // 310 S4 first-light 0.10 — real estimator ships in the same
        // commit (concealment-keyed write_wildlife_vs_cat row).
        let s = ScoringConstants::default();
        assert_eq!(s.shadowfox_hunt_cat_ambush_affordance_weight, 0.10);
        let dse = ShadowfoxHuntDse::new(&s);
        assert_eq!(dse.considerations().len(), 4);
        assert!((dse.composition().weights[0] - 0.6 * 0.9).abs() < 1e-4);
        assert!((dse.composition().weights[3] - 0.10).abs() < 1e-4);
    }

    #[test]
    fn ambush_affordance_axis_absent_when_zeroed() {
        let mut s = ScoringConstants::default();
        s.shadowfox_hunt_cat_ambush_affordance_weight = 0.0;
        let dse = ShadowfoxHuntDse::new(&s);
        assert!(dse.considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == CAT_AMBUSH_AFFORDANCE_INPUT
        )));
    }
}
