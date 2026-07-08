//! Snake `Foraging` — active prey pursuit. Fires when hunger is
//! acute (steep logistic, midpoint 0.3 — only when very hungry)
//! and modulated by aggression personality.
//!
//! `WeightedSum` of two axes — `hunger_urgency` via `Logistic(8,
//! 0.3)` (steep, fires only under acute hunger), `aggression` via
//! `Linear(1.0, 0.0)` (aggressive snakes forage more readily).
//!
//! 265 adds a conditional `best_prey_stalk_affordance` axis (active
//! at 0.0) — max `Affordance(Stalk, snake, prey)` over prey in
//! detection range, from substrate 261.
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
pub const AGGRESSION_INPUT: &str = "aggression";
/// 265: max `Affordance(Stalk, snake, prey)` over prey in detection
/// range. Populated by `snake_goap::snake_evaluate_and_plan`;
/// wildlife-vs-prey writer rows arrive with ticket 314.
pub const STALK_AFFORDANCE_INPUT: &str = "best_prey_stalk_affordance";

pub struct SnakeForagingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SnakeForagingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let hunger_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.3,
        };
        let aggression_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        let mut considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(HUNGER_URGENCY_INPUT, hunger_curve)),
            Consideration::Scalar(ScalarConsideration::new(AGGRESSION_INPUT, aggression_curve)),
        ];
        let mut weights = vec![0.7, 0.3];

        // 265: conditional stalk-affordance axis, active at
        // first-light 0.10 since plan step 21
        // (the 264 socialize_target shape). Base two scale by
        // `(1 − extra)` so the WeightedSum stays at 1.0.
        let affordance_w = scoring.snake_forage_stalk_affordance_weight.clamp(0.0, 1.0);
        if affordance_w > 0.0 {
            let scale = 1.0 - affordance_w;
            for w in &mut weights {
                *w *= scale;
            }
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                STALK_AFFORDANCE_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(affordance_w);
        }

        Self {
            id: DseId("snake_foraging"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for SnakeForagingDse {
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
            state: GoalState::predicate("snake_fed_by_foraging", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn snake_foraging_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(SnakeForagingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_foraging_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(SnakeForagingDse::new(&s).id().0, "snake_foraging");
    }

    #[test]
    fn snake_foraging_has_two_axes() {
        // Pinned via the explicitly-zeroed 265 weight (config-override
        // escape hatch); at active defaults the conditional axis makes
        // three.
        let mut s = ScoringConstants::default();
        s.snake_forage_stalk_affordance_weight = 0.0;
        assert_eq!(SnakeForagingDse::new(&s).considerations().len(), 2);
    }

    #[test]
    fn snake_foraging_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = SnakeForagingDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn stalk_affordance_axis_absent_when_zeroed() {
        // 265 activation: zeroing the weight (config-override escape
        // hatch) MUST rebuild the pre-265 two-axis composition
        // byte-identically.
        let mut s = ScoringConstants::default();
        s.snake_forage_stalk_affordance_weight = 0.0;
        let dse = SnakeForagingDse::new(&s);
        assert_eq!(dse.considerations().len(), 2);
        assert!(dse.considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == STALK_AFFORDANCE_INPUT
        )));
    }

    #[test]
    fn stalk_affordance_axis_present_and_renormalized_when_active() {
        let mut s = ScoringConstants::default();
        s.snake_forage_stalk_affordance_weight = 0.2;
        let dse = SnakeForagingDse::new(&s);
        assert_eq!(dse.considerations().len(), 3);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
        assert!((dse.composition().weights[0] - 0.7 * 0.8).abs() < 1e-4);
        assert!((dse.composition().weights[2] - 0.2).abs() < 1e-4);
    }

    #[test]
    fn axis_active_at_default_first_light() {
        // 265 activation (plan step 21): first-light 0.10.
        let s = ScoringConstants::default();
        assert_eq!(s.snake_forage_stalk_affordance_weight, 0.10);
        let dse = SnakeForagingDse::new(&s);
        assert_eq!(dse.considerations().len(), 3);
        assert!(dse.considerations().iter().any(|c| matches!(
            c,
            Consideration::Scalar(sc) if sc.name == STALK_AFFORDANCE_INPUT
        )));
        assert!((dse.composition().weights[0] - 0.7 * 0.9).abs() < 1e-4);
        assert!((dse.composition().weights[2] - 0.10).abs() < 1e-4);
    }
}
