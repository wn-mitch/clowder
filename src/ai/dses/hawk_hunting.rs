//! Hawk `Hunting` — hunger-driven prey pursuit.
//!
//! `WeightedSum` of two axes — `hunger_urgency` via `Logistic(6, 0.5)`
//! (sigmoid ramp centered at half-hungry), `prey_nearby` via
//! `Linear(1.0, 0.0)` (proportional to visible prey density).
//!
//! 265 adds a conditional `best_prey_predation_affordance` axis
//! (active at first-light 0.10 since plan step 21) — max
//! `Affordance(Dive|Chase, hawk, prey)` over
//! prey in detection range, from substrate 261.
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
pub const PREY_NEARBY_INPUT: &str = "prey_nearby";
/// 265: max `Affordance(Dive|Chase, hawk, prey)` over prey in
/// detection range. Populated by `hawk_goap::hawk_evaluate_and_plan`;
/// wildlife-vs-prey writer rows arrive with ticket 314.
pub const PREY_AFFORDANCE_INPUT: &str = "best_prey_predation_affordance";

pub struct HawkHuntingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HawkHuntingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let hunger_curve = Curve::Logistic {
            steepness: 6.0,
            midpoint: 0.5,
        };
        let prey_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        let mut considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(HUNGER_URGENCY_INPUT, hunger_curve)),
            Consideration::Scalar(ScalarConsideration::new(PREY_NEARBY_INPUT, prey_curve)),
        ];
        let mut weights = vec![0.7, 0.3];

        // 265: conditional predation-affordance axis, active at
        // first-light 0.10 since plan step 21
        // (the 264 socialize_target shape). Base two scale by
        // `(1 − extra)` so the WeightedSum stays at 1.0.
        let affordance_w = scoring.hawk_hunting_prey_affordance_weight.clamp(0.0, 1.0);
        if affordance_w > 0.0 {
            let scale = 1.0 - affordance_w;
            for w in &mut weights {
                *w *= scale;
            }
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                PREY_AFFORDANCE_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(affordance_w);
        }

        Self {
            id: DseId("hawk_hunting"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for HawkHuntingDse {
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
            state: GoalState::predicate("hawk_fed", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn hawk_hunting_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(HawkHuntingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hawk_hunting_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(HawkHuntingDse::new(&s).id().0, "hawk_hunting");
    }

    #[test]
    fn hawk_hunting_has_two_axes() {
        // Pinned via the explicitly-zeroed 265 weight (config-override
        // escape hatch); at active defaults the conditional
        // prey-affordance axis makes three.
        let mut s = ScoringConstants::default();
        s.hawk_hunting_prey_affordance_weight = 0.0;
        assert_eq!(HawkHuntingDse::new(&s).considerations().len(), 2);
    }

    #[test]
    fn hawk_hunting_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = HawkHuntingDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn hawk_hunting_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            HawkHuntingDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn hawk_hunting_maslow_tier_is_one() {
        let s = ScoringConstants::default();
        assert_eq!(HawkHuntingDse::new(&s).maslow_tier(), 1);
    }

    #[test]
    fn prey_affordance_axis_absent_when_zeroed() {
        // 265 activation: zeroing the weight (config-override escape
        // hatch) MUST rebuild the pre-265 two-axis composition
        // byte-identically.
        let mut s = ScoringConstants::default();
        s.hawk_hunting_prey_affordance_weight = 0.0;
        let dse = HawkHuntingDse::new(&s);
        assert_eq!(dse.considerations().len(), 2);
        assert!(dse.considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == PREY_AFFORDANCE_INPUT
        )));
    }

    #[test]
    fn prey_affordance_axis_active_at_default() {
        // 265 activation (plan step 21): first-light 0.10.
        let s = ScoringConstants::default();
        assert_eq!(s.hawk_hunting_prey_affordance_weight, 0.10);
        let dse = HawkHuntingDse::new(&s);
        assert_eq!(dse.considerations().len(), 3);
        assert!(dse.considerations().iter().any(|c| matches!(
            c,
            Consideration::Scalar(sc) if sc.name == PREY_AFFORDANCE_INPUT
        )));
        assert!((dse.composition().weights[0] - 0.7 * 0.9).abs() < 1e-4);
        assert!((dse.composition().weights[2] - 0.10).abs() < 1e-4);
    }

    #[test]
    fn prey_affordance_axis_present_and_renormalized_when_active() {
        let mut s = ScoringConstants::default();
        s.hawk_hunting_prey_affordance_weight = 0.2;
        let dse = HawkHuntingDse::new(&s);
        assert_eq!(dse.considerations().len(), 3);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
        assert!((dse.composition().weights[0] - 0.7 * 0.8).abs() < 1e-4);
        assert!((dse.composition().weights[2] - 0.2).abs() < 1e-4);
    }
}
