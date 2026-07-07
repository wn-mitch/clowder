//! Snake `Ambushing` — sit-and-wait predation strategy. Snakes
//! coil near prey trails and strike when hungry enough and patience
//! is high.
//!
//! `WeightedSum` of two axes — `hunger_urgency` via `Logistic(5,
//! 0.5)` (moderate ramp centered at half-hunger), `patience` via
//! `Linear(1.0, 0.0)` (personality modulator — patient snakes
//! prefer ambush over active foraging).
//!
//! 265 adds a conditional `best_prey_strike_affordance` axis
//! (dormant at 0.0) — max `Affordance(Strike, snake, prey)` over prey
//! in detection range. Strike is adjacency-gated in the 261 writer,
//! so the axis rewards holding an ambush spot prey actually pass.
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
pub const PATIENCE_INPUT: &str = "patience";
/// 265: max `Affordance(Strike, snake, prey)` over prey in detection
/// range. Populated by `snake_goap::snake_evaluate_and_plan`;
/// wildlife-vs-prey writer rows arrive with ticket 314.
pub const STRIKE_AFFORDANCE_INPUT: &str = "best_prey_strike_affordance";

pub struct SnakeAmbushingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SnakeAmbushingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let hunger_curve = Curve::Logistic {
            steepness: 5.0,
            midpoint: 0.5,
        };
        let patience_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        let mut considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(HUNGER_URGENCY_INPUT, hunger_curve)),
            Consideration::Scalar(ScalarConsideration::new(PATIENCE_INPUT, patience_curve)),
        ];
        let mut weights = vec![0.6, 0.4];

        // 265: conditional strike-affordance axis, dormant at 0.0
        // (the 264 socialize_target shape). Base two scale by
        // `(1 − extra)` so the WeightedSum stays at 1.0.
        let affordance_w = scoring
            .snake_ambush_strike_affordance_weight
            .clamp(0.0, 1.0);
        if affordance_w > 0.0 {
            let scale = 1.0 - affordance_w;
            for w in &mut weights {
                *w *= scale;
            }
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                STRIKE_AFFORDANCE_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(affordance_w);
        }

        Self {
            id: DseId("snake_ambushing"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for SnakeAmbushingDse {
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
            state: GoalState::predicate("snake_fed_by_ambush", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn snake_ambushing_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(SnakeAmbushingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_ambushing_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(SnakeAmbushingDse::new(&s).id().0, "snake_ambushing");
    }

    #[test]
    fn snake_ambushing_has_two_axes() {
        let s = ScoringConstants::default();
        assert_eq!(SnakeAmbushingDse::new(&s).considerations().len(), 2);
    }

    #[test]
    fn snake_ambushing_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = SnakeAmbushingDse::new(&s)
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn snake_ambushing_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            SnakeAmbushingDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn strike_affordance_axis_absent_at_default() {
        // 265: weight ships at 0.0; the axis MUST NOT appear and the
        // two-axis composition is byte-identical to pre-265.
        let s = ScoringConstants::default();
        assert_eq!(s.snake_ambush_strike_affordance_weight, 0.0);
        let dse = SnakeAmbushingDse::new(&s);
        assert_eq!(dse.considerations().len(), 2);
        assert!(dse.considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == STRIKE_AFFORDANCE_INPUT
        )));
    }

    #[test]
    fn strike_affordance_axis_present_and_renormalized_when_active() {
        let mut s = ScoringConstants::default();
        s.snake_ambush_strike_affordance_weight = 0.25;
        let dse = SnakeAmbushingDse::new(&s);
        assert_eq!(dse.considerations().len(), 3);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
        assert!((dse.composition().weights[0] - 0.6 * 0.75).abs() < 1e-4);
        assert!((dse.composition().weights[2] - 0.25).abs() < 1e-4);
    }
}
