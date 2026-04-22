//! Fox `Resting` — Rest-urgency peer (§3.3.2 anchor = 1.0). The DSE
//! that dominated the fox GOAP plan distribution after Phase 3c.2
//! (35k Resting plans vs 0 Hunting plans at a 5-min smoke soak)
//! because its un-ported peak (~1.18) exceeded every compressed
//! peer's 1.0 ceiling. Porting restores the peer-group contract.
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of three axes —
//! `hunger` (satiation scalar, bounded `[0, 1]` — §2.3's "well-fed
//! produces comfort"), `health_fraction` (Linear), `day_phase` via
//! `Piecewise` on `fox_rest_{dawn,day,dusk,night}_bonus`.
//!
//! **Bilinear-vs-additive port.** Old inline:
//! `(hunger × health_fraction × 0.6 + phase_bonus) × l1`. The
//! §3.1.1 row (line 1518) specifies WS with three separate axes —
//! intentionally *additive* rather than bilinear, so a well-rested
//! fox in day-phase rests by day "even when comfort is low," per
//! the row's design note. We honor the §3.1.1 composition choice.
//!
//! **Magnitude.** Old peak ≈ `1.0 × 1.0 × 0.6 + 0.9 ≈ 1.5`; new
//! peak = 1.0 under WS with weights summing to 1.0. Matches the
//! §3.3.2 Rest anchor.
//!
//! **Eligibility gate.** `has_den && hunger > 0.5` stays outer in
//! `score_fox_dispositions` — §4 marker port is Phase 3d.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{piecewise, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const HUNGER_INPUT: &str = "hunger";
pub const HEALTH_FRACTION_INPUT: &str = "health_fraction";
pub const DAY_PHASE_INPUT: &str = "day_phase";

// Phase-to-knot encoding shared across all fox DSEs + Sleep. Keep in
// sync with `fox_hunting` / `sleep` / `fox_scoring::day_phase_scalar`.
pub const DAWN_KNOT: f32 = 0.0;
pub const DAY_KNOT: f32 = 0.33;
pub const DUSK_KNOT: f32 = 0.66;
pub const NIGHT_KNOT: f32 = 1.0;

pub struct FoxRestingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxRestingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let day_phase_curve = piecewise(vec![
            (DAWN_KNOT, scoring.fox_rest_dawn_bonus),
            (DAY_KNOT, scoring.fox_rest_day_bonus),
            (DUSK_KNOT, scoring.fox_rest_dusk_bonus),
            (NIGHT_KNOT, scoring.fox_rest_night_bonus),
        ]);
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("fox_resting"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HEALTH_FRACTION_INPUT, linear)),
                Consideration::Scalar(ScalarConsideration::new(DAY_PHASE_INPUT, day_phase_curve)),
            ],
            // RtEO sum = 1.0. Day phase dominates — diurnal foxes
            // rest by day independent of comfort state (§3.1.1 row
            // 1518 design note). Hunger + health carry the "well-fed
            // + healthy produces comfort" signal but split the
            // remaining weight equally.
            composition: Composition::weighted_sum(vec![0.25, 0.25, 0.5]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for FoxRestingDse {
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
                label: "fox_rested_at_den",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn fox_resting_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(FoxRestingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_resting_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(FoxRestingDse::new(&s).id().0, "fox_resting");
    }

    #[test]
    fn fox_resting_has_three_axes() {
        let s = ScoringConstants::default();
        assert_eq!(FoxRestingDse::new(&s).considerations().len(), 3);
    }

    #[test]
    fn fox_resting_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            FoxRestingDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn fox_resting_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = FoxRestingDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn fox_resting_maslow_tier_is_one() {
        let s = ScoringConstants::default();
        assert_eq!(FoxRestingDse::new(&s).maslow_tier(), 1);
    }

    #[test]
    fn day_phase_knots_match_fox_rest_constants() {
        let s = ScoringConstants::default();
        let dse = FoxRestingDse::new(&s);
        let c = match &dse.considerations()[2] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!((c.evaluate(DAY_KNOT) - s.fox_rest_day_bonus).abs() < 1e-4);
        assert!((c.evaluate(NIGHT_KNOT) - s.fox_rest_night_bonus).abs() < 1e-4);
    }
}
