//! `Sleep` — Rest-urgency peer (§3.3.2 anchor = 1.0). Cross-species
//! peer of fox `Resting` through the Rest peer group.
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of three axes — `energy_deficit`
//! via `sleep_dep()` (Logistic(10, 0.7) — the catalog's steepest
//! aside from flee-or-fight, per §2.3 "micro-sleeps are involuntary
//! past ~30%"), `day_phase` via `Piecewise` on
//! `sleep_{dawn,day,dusk,night}_bonus`, `injury_rest` via
//! `Linear(slope=injury_rest_bonus)` on health_deficit.
//!
//! The WS composition preserves the design intent captured in the
//! old inline comment at `scoring.rs:212–214`: *"Additive (not
//! multiplicative) so Sleep remains available as a pressure-release
//! valve at low energy even during feeding peaks."*
//!
//! **Magnitude compression.** Old inline peak:
//! `1.2 + sleep_night_bonus + injury_rest_bonus ≈ 2.8`. Under WS
//! with weights summing to 1.0, peak compresses to 1.0 — matching
//! the Rest peer-group anchor. Cross-peer-group ordinals vs.
//! starvation/fatal-threat hold because those groups also anchor
//! at 1.0.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{piecewise, sleep_dep, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const ENERGY_DEFICIT_INPUT: &str = "energy_deficit";
pub const DAY_PHASE_INPUT: &str = "day_phase";
pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";
/// Ticket 087 — interoceptive perception axis. Wounded cats accumulate
/// `pain_level` from `Health.injuries` (severity sum normalized) and
/// score Sleep higher so the `Resting` disposition wins the contest
/// before the disposition-layer critical-health interrupt fires.
pub const PAIN_LEVEL_INPUT: &str = "pain_level";

/// §L2.10.7 Sleep range — Manhattan tiles for the
/// own-sleeping-spot anchor. 15 ≈ a few-room radius; cats farther
/// from a sleeping spot find sleeping unattractive (sharp Power
/// fall-off — 'Strong preference for own den; sharp fall-off',
/// spec line 5622).
pub const SLEEP_SPOT_RANGE: f32 = 15.0;

// Phase-to-knot encoding; must match `fox_hunting` + the scoring-layer
// `day_phase_scalar` encoder.
pub const DAWN_KNOT: f32 = 0.0;
pub const DAY_KNOT: f32 = 0.33;
pub const DUSK_KNOT: f32 = 0.66;
pub const NIGHT_KNOT: f32 = 1.0;

pub struct SleepDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SleepDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let day_phase_curve = piecewise(vec![
            (DAWN_KNOT, scoring.sleep_dawn_bonus),
            (DAY_KNOT, scoring.sleep_day_bonus),
            (DUSK_KNOT, scoring.sleep_dusk_bonus),
            (NIGHT_KNOT, scoring.sleep_night_bonus),
        ]);
        // Injury rest: old formula `(1 - health) * injury_rest_bonus`
        // gated on `health < 1.0`. The gate is implicit here — at
        // health=1, deficit=0, Linear output is 0.
        let injury_curve = Curve::Linear {
            slope: scoring.injury_rest_bonus,
            intercept: 0.0,
        };

        // §L2.10.7 row Sleep: Power-Invert curve over distance to
        // the cat's own sleeping spot. Spec line 5622: 'Strong
        // preference for own den; sharp fall-off from it.' Power
        // gives that sharper fall-off than Logistic (faster decay
        // beyond the bucket midpoint).
        let spot_distance = Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        };
        // Ticket 087 — `pain_level` axis. Linear curve over the
        // interoceptive `pain_level` scalar (sum of unhealed-injury
        // severities normalized into [0, 1]). Same Linear shape as the
        // pre-existing `injury_rest` axis but driven by injury *count
        // and severity* rather than health-ratio deficit, so a cat with
        // multiple wounds at otherwise-restored HP still scores Sleep
        // up. Pairs with the `health_deficit` axis (HP-ratio-driven)
        // for the cumulative "I am hurt" signal.
        let pain_curve = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        Self {
            id: DseId("sleep"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(ENERGY_DEFICIT_INPUT, sleep_dep())),
                Consideration::Scalar(ScalarConsideration::new(DAY_PHASE_INPUT, day_phase_curve)),
                Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, injury_curve)),
                Consideration::Scalar(ScalarConsideration::new(PAIN_LEVEL_INPUT, pain_curve)),
                Consideration::Spatial(SpatialConsideration::new(
                    "sleep_spot_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::OwnSleepingSpot),
                    SLEEP_SPOT_RANGE,
                    spot_distance,
                )),
            ],
            // Ticket 087 — original four weights [0.40, 0.24, 0.16, 0.20]
            // sum to 1.0. Adding `pain_level` at weight 0.10 — sized
            // small enough that uninjured cats score Sleep identically
            // (pain_level = 0 → axis contributes 0), large enough that
            // a cat with multiple wounds gets a meaningful bump. The
            // four originals scale by 0.90 so the sum stays at 1.0.
            composition: Composition::weighted_sum(vec![
                0.40 * 0.90,
                0.24 * 0.90,
                0.16 * 0.90,
                0.10,
                0.20 * 0.90,
            ]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for SleepDse {
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
        // §7.3: Sleep is a constituent action of the Resting
        // disposition and rides Resting's `Blind` strategy. The
        // Maslow gate handles preemption; AI8 caps runaway sleeps.
        CommitmentStrategy::Blind
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "energy_restored",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::Blind,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn sleep_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(SleepDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(SleepDse::new(&s).id().0, "sleep");
    }

    #[test]
    fn sleep_has_five_axes() {
        // §L2.10.7 + ticket 087: energy + day_phase + injury_rest +
        // pain_level + spot_distance.
        let s = ScoringConstants::default();
        assert_eq!(SleepDse::new(&s).considerations().len(), 5);
    }

    #[test]
    fn sleep_uses_own_sleeping_spot_anchor() {
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "sleep_spot_distance" => Some(sp),
                _ => None,
            })
            .expect("sleep_spot_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::OwnSleepingSpot)
        ));
    }

    #[test]
    fn sleep_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            SleepDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn sleep_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = SleepDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn sleep_maslow_tier_is_one() {
        let s = ScoringConstants::default();
        assert_eq!(SleepDse::new(&s).maslow_tier(), 1);
    }

    fn scalar_axis<'a>(dse: &'a SleepDse, name: &str) -> &'a Curve {
        dse.considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(sc) if sc.name == name => Some(&sc.curve),
                _ => None,
            })
            .unwrap_or_else(|| panic!("scalar axis {name} must exist"))
    }

    #[test]
    fn injury_curve_zero_at_full_health() {
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = scalar_axis(&dse, HEALTH_DEFICIT_INPUT);
        // health_deficit = 0 (full health) → Linear output = 0.
        assert!((c.evaluate(0.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn day_phase_knots_match_scoring_constants() {
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = scalar_axis(&dse, DAY_PHASE_INPUT);
        assert!((c.evaluate(DAWN_KNOT) - s.sleep_dawn_bonus).abs() < 1e-4);
        assert!((c.evaluate(NIGHT_KNOT) - s.sleep_night_bonus).abs() < 1e-4);
    }
}
