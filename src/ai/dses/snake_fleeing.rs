//! Snake `Fleeing` — retreat from threats. Snakes are timid; even
//! a single nearby cat saturates the threat signal.
//!
//! `WeightedSum` of two axes — `health_deficit` via `Logistic(8,
//! 0.5)` (injury-panic threshold), `cats_nearby` via `Linear(1.0,
//! 0.0)` (saturates at 1 since input is 0-1 from the scalar map —
//! one cat is enough to provoke flight).
//!
//! 265 adds a conditional `perceived_cat_threat` axis (dormant at 0.0)
//! — max `CatBeliefs[cat].perceived_violence_capability` over cats in
//! detection range, the snake's own belief about the danger around it.
//!
//! Maslow tier 1 — survival (escape).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";
pub const CATS_NEARBY_INPUT: &str = "cats_nearby";
/// 265: max `CatBeliefs[cat].perceived_violence_capability` over cats
/// in detection range (implanted from `cat_perceived_by_snake`, updated
/// by witnessed Attack/Hunt evidence). Populated by
/// `snake_goap::snake_evaluate_and_plan`.
pub const PERCEIVED_CAT_THREAT_INPUT: &str = "perceived_cat_threat";

pub struct SnakeFleeingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SnakeFleeingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let health_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        // Input is already 0-1 from the scalar map; Linear(1,0)
        // passes it through directly.
        let cats_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        let mut considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, health_curve)),
            Consideration::Scalar(ScalarConsideration::new(CATS_NEARBY_INPUT, cats_curve)),
        ];
        let mut weights = vec![0.5, 0.5];

        // 265: conditional belief axis, dormant at 0.0 (the 264
        // socialize_target shape). Base two scale by `(1 − extra)`
        // so the WeightedSum stays at 1.0.
        let belief_w = scoring
            .snake_flee_cat_violence_belief_weight
            .clamp(0.0, 1.0);
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
            id: DseId("snake_fleeing"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for SnakeFleeingDse {
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
            state: GoalState::predicate("snake_fled_to_safety", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn snake_fleeing_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(SnakeFleeingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_fleeing_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(SnakeFleeingDse::new(&s).id().0, "snake_fleeing");
    }

    #[test]
    fn snake_fleeing_has_two_axes() {
        let s = ScoringConstants::default();
        assert_eq!(SnakeFleeingDse::new(&s).considerations().len(), 2);
    }

    #[test]
    fn snake_fleeing_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = SnakeFleeingDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn cat_threat_axis_absent_at_default() {
        // 265: weight ships at 0.0; the axis MUST NOT appear and the
        // two-axis composition is byte-identical to pre-265.
        let s = ScoringConstants::default();
        assert_eq!(s.snake_flee_cat_violence_belief_weight, 0.0);
        let dse = SnakeFleeingDse::new(&s);
        assert_eq!(dse.considerations().len(), 2);
        assert!(dse.considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == PERCEIVED_CAT_THREAT_INPUT
        )));
    }

    #[test]
    fn cat_threat_axis_present_and_renormalized_when_active() {
        let mut s = ScoringConstants::default();
        s.snake_flee_cat_violence_belief_weight = 0.2;
        let dse = SnakeFleeingDse::new(&s);
        assert_eq!(dse.considerations().len(), 3);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
        assert!((dse.composition().weights[0] - 0.5 * 0.8).abs() < 1e-4);
        assert!((dse.composition().weights[2] - 0.2).abs() < 1e-4);
    }
}
