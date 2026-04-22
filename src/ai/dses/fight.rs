//! `Fight` — Fatal-threat peer (§3.3.2 anchor = 1.0). The highest-
//! axis-count DSE in the catalog; its shape is the spec's motivating
//! example for why `WeightedSum`/RtEO exists.
//!
//! Per §2.3 + §3.1.1: 5 axes, WS composition. Boldness + combat
//! proficiency are the *driver* axes; health + safety piecewise
//! suppressions are *gating* modulators; ally_count is the
//! group-courage social signal. A pure CP would let any near-zero
//! axis collapse the signal, erasing the "low-boldness cat swept
//! along by allies" archetype — WS keeps the group signal.
//!
//! **Shape vs. inline.** Old formula multiplies boldness × combat ×
//! health_piece × safety_piece × l2 then *adds* a group bonus
//! (`allies × fight_ally_bonus_per_cat`). Port folds the group
//! bonus into the WS axis list; under RtEO this mixes cleanly.
//! Peak drops from `boldness_scale ≈ 1.5 + group_bonus` to 1.0,
//! matching the peer-group anchor.
//!
//! **Eligibility gate.** `has_threat_nearby && allies_fighting_threat
//! ≥ fight_min_allies` stays outer in `score_actions`.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{fight_gating, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const BOLDNESS_INPUT: &str = "boldness";
pub const COMBAT_EFFECTIVE_INPUT: &str = "combat_effective";
pub const HEALTH_INPUT: &str = "health";
pub const SAFETY_INPUT: &str = "safety";
pub const ALLY_COUNT_INPUT: &str = "ally_count";

pub struct FightDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FightDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // Saturating-count curve for ally_count (§2.3 anchor). Slope
        // is the per-ally bonus; ClampMax at 1.0 caps contribution so
        // the axis stays within the peer-group ceiling.
        let ally_count_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: scoring.fight_ally_bonus_per_cat,
                intercept: 0.0,
            }),
            post: PostOp::ClampMax(1.0),
        };

        Self {
            id: DseId("fight"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    BOLDNESS_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    COMBAT_EFFECTIVE_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(HEALTH_INPUT, fight_gating())),
                Consideration::Scalar(ScalarConsideration::new(SAFETY_INPUT, fight_gating())),
                Consideration::Scalar(ScalarConsideration::new(ALLY_COUNT_INPUT, ally_count_curve)),
            ],
            // RtEO weights sum to 1.0.
            //   - boldness 0.25: the temperament driver.
            //   - combat_effective 0.20: capability; low-skill cats
            //     still fight when bold + flocked.
            //   - health 0.15, safety 0.15: Piecewise suppressions —
            //     not a hard gate but a strong dampener when hurt /
            //     cornered.
            //   - ally_count 0.25: group-courage signal per §3.1.1.
            composition: Composition::weighted_sum(vec![0.25, 0.20, 0.15, 0.15, 0.25]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for FightDse {
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
                label: "threat_defeated",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn fight_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(FightDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fight_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(FightDse::new(&s).id().0, "fight");
    }

    #[test]
    fn fight_has_five_axes() {
        let s = ScoringConstants::default();
        assert_eq!(FightDse::new(&s).considerations().len(), 5);
    }

    #[test]
    fn fight_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = FightDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
    }

    #[test]
    fn fight_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            FightDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn fight_maslow_tier_is_two() {
        let s = ScoringConstants::default();
        assert_eq!(FightDse::new(&s).maslow_tier(), 2);
    }

    #[test]
    fn ally_count_curve_saturates() {
        let s = ScoringConstants::default();
        let dse = FightDse::new(&s);
        let c = match &dse.considerations()[4] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // slope = fight_ally_bonus_per_cat; 1 ally = 0.15, 10 allies
        // saturates at 1.0.
        let bonus = s.fight_ally_bonus_per_cat;
        assert!((c.evaluate(1.0) - bonus).abs() < 1e-4);
        assert!((c.evaluate(10.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn health_piecewise_suppresses_injured_cat() {
        let s = ScoringConstants::default();
        let dse = FightDse::new(&s);
        let c = match &dse.considerations()[2] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // fight_gating: (0,0), (0.3,0.2), (0.5,1.0), (1.0,1.0).
        assert!(c.evaluate(0.0) < 0.01);
        assert!(c.evaluate(0.3) < 0.25);
        assert!((c.evaluate(0.5) - 1.0).abs() < 0.01);
        assert!((c.evaluate(1.0) - 1.0).abs() < 0.01);
    }
}
