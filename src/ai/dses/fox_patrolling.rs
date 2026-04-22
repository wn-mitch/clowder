//! Fox `Patrolling` — Territory-urgency peer (§3.3.2 anchor = 1.0).
//! Cross-species peer of cat `Patrol`.
//!
//! Per §2.3 + §3.1.1 row 1525: `WeightedSum` of 4 axes —
//! `territory_scent_deficit` via `Logistic(5, 0.5)` (scent-marking
//! urgency rises as marks fade; gentler than hangry's steepness=8 —
//! foxes don't panic about territory), `time_since_patrol` via
//! `Composite { Linear(divisor=2000), Clamp(max=1.0) }`
//! (saturating-count anchor), `day_phase` via `Piecewise` on
//! `fox_patrol_{dawn,day,dusk,night}_bonus` (Patrol knots distinct
//! from Hunt/Rest), `territoriality` via Linear.
//!
//! Eligibility: `has_den` (outer gate). Maslow tier 2.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{piecewise, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const TERRITORY_SCENT_DEFICIT_INPUT: &str = "territory_scent_deficit";
pub const TICKS_SINCE_PATROL_INPUT: &str = "ticks_since_patrol";
pub const DAY_PHASE_INPUT: &str = "day_phase";
pub const TERRITORIALITY_INPUT: &str = "territoriality";

// Shared knot-x encoding across species — see `dses::fox_hunting` +
// `dses::sleep` + `dses::fox_resting`.
pub const DAWN_KNOT: f32 = 0.0;
pub const DAY_KNOT: f32 = 0.33;
pub const DUSK_KNOT: f32 = 0.66;
pub const NIGHT_KNOT: f32 = 1.0;

pub struct FoxPatrollingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxPatrollingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let day_phase_curve = piecewise(vec![
            (DAWN_KNOT, scoring.fox_patrol_dawn_bonus),
            (DAY_KNOT, scoring.fox_patrol_day_bonus),
            (DUSK_KNOT, scoring.fox_patrol_dusk_bonus),
            (NIGHT_KNOT, scoring.fox_patrol_night_bonus),
        ]);
        // Saturating time-since-patrol: divide by 2000 then clamp at
        // 1.0. Linear can't divide directly; use slope = 1/2000 which
        // yields x/2000 for any input x (clamped to [0,1] by Linear
        // primitive). ClampMax 1.0 is effectively a no-op but names
        // the intent.
        let time_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 1.0 / 2000.0,
                intercept: 0.0,
            }),
            post: PostOp::ClampMax(1.0),
        };
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("fox_patrolling"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    TERRITORY_SCENT_DEFICIT_INPUT,
                    Curve::Logistic {
                        steepness: 5.0,
                        midpoint: 0.5,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    TICKS_SINCE_PATROL_INPUT,
                    time_curve,
                )),
                Consideration::Scalar(ScalarConsideration::new(DAY_PHASE_INPUT, day_phase_curve)),
                Consideration::Scalar(ScalarConsideration::new(TERRITORIALITY_INPUT, linear)),
            ],
            // RtEO sum = 1.0. Scent deficit + time-since co-drive;
            // day_phase is the circadian rhythm; territoriality is
            // the personality modulator.
            composition: Composition::weighted_sum(vec![0.3, 0.25, 0.2, 0.25]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for FoxPatrollingDse {
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
                label: "fox_territory_marked",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn fox_patrolling_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(FoxPatrollingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_patrolling_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(FoxPatrollingDse::new(&s).id().0, "fox_patrolling");
    }

    #[test]
    fn fox_patrolling_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = FoxPatrollingDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn time_since_curve_saturates_at_2000() {
        let s = ScoringConstants::default();
        let dse = FoxPatrollingDse::new(&s);
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!((c.evaluate(0.0) - 0.0).abs() < 1e-4);
        assert!((c.evaluate(1000.0) - 0.5).abs() < 1e-4);
        assert!((c.evaluate(2000.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(5000.0) - 1.0).abs() < 1e-4);
    }
}
