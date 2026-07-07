//! Hawk `Fleeing` — injury-driven escape response.
//!
//! `WeightedSum` of two axes — `health_deficit` via `Logistic(8, 0.5)`
//! (injury-panic threshold), `boldness` via `Composite { Linear(slope=
//! 0.5), Invert }` (damped invert — timid hawks flee more).
//!
//! 265 adds a conditional `perceived_cat_threat` axis (dormant at 0.0)
//! — max `CatBeliefs[cat].perceived_violence_capability` over cats in
//! avoidance range, the hawk's own belief about the danger around it.
//!
//! Maslow tier 1 — survival (threat response).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";
pub const BOLDNESS_INPUT: &str = "boldness";
/// 265: max `CatBeliefs[cat].perceived_violence_capability` over cats
/// in avoidance range (implanted from `cat_perceived_by_hawk`, updated
/// by witnessed Attack/Hunt evidence). Populated by
/// `hawk_goap::hawk_evaluate_and_plan`.
pub const PERCEIVED_CAT_THREAT_INPUT: &str = "perceived_cat_threat";

pub struct HawkFleeingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HawkFleeingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let health_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        // Damped invert: Linear(slope=0.5) maps boldness=1.0 → 0.5,
        // then Invert gives (1 - 0.5) = 0.5. Max-bold hawk still
        // contributes 0.5; timid hawk (bold=0) contributes 1.0.
        let boldness_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 0.5,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };

        let mut considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, health_curve)),
            Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_curve)),
        ];
        let mut weights = vec![0.65, 0.35];

        // 265: conditional belief axis, dormant at 0.0 (the 264
        // socialize_target shape). Base two scale by `(1 − extra)`
        // so the WeightedSum stays at 1.0.
        let belief_w = scoring.hawk_flee_cat_violence_belief_weight.clamp(0.0, 1.0);
        if belief_w > 0.0 {
            let scale = 1.0 - belief_w;
            for w in &mut weights {
                *w *= scale;
            }
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                PERCEIVED_CAT_THREAT_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(belief_w);
        }

        Self {
            id: DseId("hawk_fleeing"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for HawkFleeingDse {
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
            state: GoalState::predicate("hawk_fled_to_safety", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn hawk_fleeing_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(HawkFleeingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hawk_fleeing_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(HawkFleeingDse::new(&s).id().0, "hawk_fleeing");
    }

    #[test]
    fn hawk_fleeing_has_two_axes() {
        let s = ScoringConstants::default();
        assert_eq!(HawkFleeingDse::new(&s).considerations().len(), 2);
    }

    #[test]
    fn hawk_fleeing_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = HawkFleeingDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn hawk_fleeing_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            HawkFleeingDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn hawk_fleeing_maslow_tier_is_one() {
        let s = ScoringConstants::default();
        assert_eq!(HawkFleeingDse::new(&s).maslow_tier(), 1);
    }

    #[test]
    fn boldness_damped_invert() {
        let s = ScoringConstants::default();
        let dse = HawkFleeingDse::new(&s);
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Linear(slope=0.5) then Invert. boldness=0 → inner=0 → invert=1.
        // boldness=1 → inner=0.5 → invert=0.5.
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn cat_threat_axis_absent_at_default() {
        // 265: weight ships at 0.0; the axis MUST NOT appear and the
        // two-axis composition is byte-identical to pre-265.
        let s = ScoringConstants::default();
        assert_eq!(s.hawk_flee_cat_violence_belief_weight, 0.0);
        let dse = HawkFleeingDse::new(&s);
        assert_eq!(dse.considerations().len(), 2);
        assert!(dse.considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == PERCEIVED_CAT_THREAT_INPUT
        )));
    }

    #[test]
    fn cat_threat_axis_present_and_renormalized_when_active() {
        let mut s = ScoringConstants::default();
        s.hawk_flee_cat_violence_belief_weight = 0.2;
        let dse = HawkFleeingDse::new(&s);
        assert_eq!(dse.considerations().len(), 3);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
        assert!((dse.composition().weights[0] - 0.65 * 0.8).abs() < 1e-4);
        assert!((dse.composition().weights[2] - 0.2).abs() < 1e-4);
    }
}
