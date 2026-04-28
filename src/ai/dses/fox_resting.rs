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
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{piecewise, Curve, PostOp};
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

/// §L2.10.7 fox Resting range — Manhattan tiles for the
/// home-den anchor. 12 ≈ a fox's territorial radius; foxes farther
/// from the den than this find resting unattractive (sharp Power
/// fall-off encodes 'home-base pull').
pub const FOX_RESTING_DEN_RANGE: f32 = 12.0;

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
        // §L2.10.7 row Resting: Power curve over distance to the
        // fox's home-den anchor. `Composite { Polynomial(exp=2,
        // divisor=1), Invert }` evaluates `1 - cost^2`: at the den
        // score=1, half-distance → 0.75, range edge → 0. Sharp
        // 'home-base pull' per spec rationale (line 5653).
        let den_distance = Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("fox_resting"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HEALTH_FRACTION_INPUT, linear)),
                Consideration::Scalar(ScalarConsideration::new(DAY_PHASE_INPUT, day_phase_curve)),
                Consideration::Spatial(SpatialConsideration::new(
                    "fox_resting_den_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::OwnDen),
                    FOX_RESTING_DEN_RANGE,
                    den_distance,
                )),
            ],
            // RtEO sum = 1.0. Day phase dominates (diurnal foxes
            // rest by day independent of comfort state — §3.1.1 row
            // 1518). Den proximity at 0.20 mirrors the §L2.10.7
            // Power-curve weight precedent. Original three weights
            // (0.25/0.25/0.5) renormalized by ×0.80 to keep sum=1.0.
            composition: Composition::weighted_sum(vec![0.20, 0.20, 0.40, 0.20]),
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
    fn fox_resting_has_four_axes() {
        // §L2.10.7: hunger + health + day_phase + den_distance.
        let s = ScoringConstants::default();
        assert_eq!(FoxRestingDse::new(&s).considerations().len(), 4);
    }

    #[test]
    fn fox_resting_uses_own_den_anchor() {
        let s = ScoringConstants::default();
        let dse = FoxRestingDse::new(&s);
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "fox_resting_den_distance" => Some(sp),
                _ => None,
            })
            .expect("fox_resting_den_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::OwnDen)
        ));
        assert!((spatial.range - FOX_RESTING_DEN_RANGE).abs() < 1e-4);
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
        // Find day_phase axis by name rather than fixed index — the
        // §L2.10.7 spatial port adds a 4th consideration after
        // day_phase.
        let day_phase_axis = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(sc) if sc.name == DAY_PHASE_INPUT => Some(&sc.curve),
                _ => None,
            })
            .expect("day_phase axis must exist");
        assert!((day_phase_axis.evaluate(DAY_KNOT) - s.fox_rest_day_bonus).abs() < 1e-4);
        assert!((day_phase_axis.evaluate(NIGHT_KNOT) - s.fox_rest_night_bonus).abs() < 1e-4);
    }
}
