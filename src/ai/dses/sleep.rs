//! `Sleep` — Rest-urgency peer (§3.3.2 anchor = 1.0). Cross-species
//! peer of fox `Resting` through the Rest peer group.
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of axes — `energy_deficit`
//! via `sleep_dep()` (Logistic(10, 0.7) — the catalog's steepest
//! aside from flee-or-fight, per §2.3 "micro-sleeps are involuntary
//! past ~30%"), `day_phase` via `Piecewise` on
//! `sleep_{dawn,day,dusk,night}_bonus`, `health_deficit` via
//! `Logistic(steepness=sleep_health_deficit_steepness,
//! midpoint=sleep_health_deficit_midpoint)` (ticket 251 — substrate-
//! side replacement for the retired `AcuteHealthAdrenalineFlee` post-
//! scoring lift).
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
//!
//! **Acute-injury urgency.** Pre-251, the `health_deficit` axis was
//! `Linear(slope=injury_rest_bonus=0.4)` and the
//! `AcuteHealthAdrenalineFlee` post-scoring modifier (047 / 119) lifted
//! the WS-clamped Sleep score by +0.5 above the [0, 1] envelope under
//! `health_deficit ≥ 0.4`. Ticket 251 retires the modifier and replaces
//! the Linear curve with a Logistic sigmoid at the same midpoint (0.4):
//! the axis lurches from ≈0 to ≈1 within ~0.1 deficit units, encoding
//! the acute-injury urgency directly in the substrate. The peak Sleep
//! score under acute injury saturates at ~1.0 (vs ~1.42 post-modifier
//! pre-251); post-232 body-state-coupled softmax (T_min ≈ 0.05) makes
//! the substrate's ~0.92→1.0 score band decisive vs ~0.4 competitors,
//! so the lost magnitude is not load-bearing for L3 ordering.

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

/// Ticket 089 — `safe_rest_distance` axis range. Tighter than
/// `SLEEP_SPOT_RANGE` (15.0) because the safe-rest signal is
/// "I'd rest here right now if I happened to be near," not "I
/// should travel across the colony to get here." 10 tiles
/// matches the home-range scale where memory-based associations
/// stay vivid.
pub const SAFE_REST_RANGE: f32 = 10.0;

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
        // Ticket 251 — Logistic curve replaces the pre-251 Linear
        // `slope=injury_rest_bonus` shape. At `health_deficit < midpoint
        // - 0.1` the axis is near-zero (healthy-cat composition near-
        // unchanged); above midpoint+0.1 the axis saturates to ~1.0,
        // contributing axis_weight × 1.0 = 0.137 under acute injury (vs
        // pre-251 contribution of 0.137 × 0.4 × 1.0 = 0.055 at full HP
        // loss). The +0.082 substrate-side bump is small in absolute
        // magnitude vs the retired modifier's +0.5 lift, but the
        // sigmoid *shape* preserves the modifier's onset semantic
        // (smoothstep transition-width ~0.1 around midpoint=0.4) and
        // post-232 body-state-coupled softmax sharpness handles the
        // L3 ordering without the modifier's amplitude.
        let injury_curve = Curve::Logistic {
            steepness: scoring.sleep_health_deficit_steepness,
            midpoint: scoring.sleep_health_deficit_midpoint,
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
        // Ticket 089 — `safe_rest_distance` axis. Same Power-Invert
        // shape as `spot_distance` but on the body-state-derived
        // safe-rest anchor (memory-suppressed-by-threats). Cats with
        // empty Sleep memory have `cat_anchors.own_safe_rest_spot =
        // None`; the spatial axis evaluates to 0.0, leaving Sleep
        // selection behaviorally identical to pre-089 for those cats.
        let safe_rest_distance = Curve::Composite {
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
                Consideration::Spatial(SpatialConsideration::new(
                    "safe_rest_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::OwnSafeRestSpot),
                    SAFE_REST_RANGE,
                    safe_rest_distance,
                )),
            ],
            // Ticket 087 — original four weights [0.40, 0.24, 0.16, 0.20]
            // sum to 1.0. Adding `pain_level` at weight 0.10 — sized
            // small enough that uninjured cats score Sleep identically
            // (pain_level = 0 → axis contributes 0), large enough that
            // a cat with multiple wounds gets a meaningful bump. The
            // four originals scale by 0.90 so the sum stays at 1.0.
            //
            // Ticket 089 — adding `safe_rest_distance` at weight 0.05;
            // existing five weights scale by 0.95 to keep the sum at
            // 1.0. The 0.05 is sized so cats with empty Sleep memory
            // (axis = 0.0 via `None` resolver path) score Sleep
            // identically to pre-089.
            composition: Composition::weighted_sum(vec![
                0.40 * 0.90 * 0.95,
                0.24 * 0.90 * 0.95,
                0.16 * 0.90 * 0.95,
                0.10 * 0.95,
                0.20 * 0.90 * 0.95,
                0.05,
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
impl crate::ai::dse::CatDse for SleepDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Sleep
    }

    fn always_emit_zero(&self) -> bool {
        true
    }
}


pub fn sleep_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
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
    fn sleep_has_six_axes() {
        // §L2.10.7 + ticket 087 + ticket 089: energy + day_phase +
        // injury_rest + pain_level + spot_distance + safe_rest_distance.
        let s = ScoringConstants::default();
        assert_eq!(SleepDse::new(&s).considerations().len(), 6);
    }

    #[test]
    fn sleep_uses_own_safe_rest_spot_anchor() {
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "safe_rest_distance" => Some(sp),
                _ => None,
            })
            .expect("safe_rest_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::OwnSafeRestSpot)
        ));
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
    fn injury_curve_near_zero_at_full_health() {
        // Ticket 251 — Logistic at deficit=0 with midpoint=0.4,
        // steepness=10 evaluates to 1/(1+exp(4)) ≈ 0.018 — small enough
        // that healthy-cat Sleep composition is near-unchanged from the
        // pre-251 Linear semantic (which was exactly 0 at full health).
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = scalar_axis(&dse, HEALTH_DEFICIT_INPUT);
        assert!(
            c.evaluate(0.0) < 0.05,
            "injury curve at full health must be <0.05, got {}",
            c.evaluate(0.0)
        );
    }

    #[test]
    fn injury_curve_low_below_threshold() {
        // Ticket 251 — at mild injury (deficit=0.3, below midpoint),
        // Logistic output stays well under 0.5 — the cat does not
        // sigmoid-lurch into Sleep until it crosses the inflection.
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = scalar_axis(&dse, HEALTH_DEFICIT_INPUT);
        assert!(
            c.evaluate(0.3) < 0.30,
            "injury curve at mild injury (deficit=0.3) must be <0.30, got {}",
            c.evaluate(0.3)
        );
    }

    #[test]
    fn injury_curve_lurch_at_midpoint() {
        // Ticket 251 — at the sigmoid inflection (deficit = midpoint),
        // Logistic outputs exactly 0.5. Pins the smoothstep-equivalent
        // half-way point of the lurch, matching the retired modifier's
        // ramp midpoint.
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = scalar_axis(&dse, HEALTH_DEFICIT_INPUT);
        let v = c.evaluate(s.sleep_health_deficit_midpoint);
        assert!(
            (v - 0.5).abs() < 1e-3,
            "injury curve at midpoint must be ≈0.5, got {v}"
        );
    }

    #[test]
    fn injury_curve_saturates_above_band() {
        // Ticket 251 — above midpoint + ~0.1 (the Logistic transition
        // band), the axis saturates to ≈1.0, matching the retired
        // modifier's smoothstep ramp = 1.0 above transition band.
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = scalar_axis(&dse, HEALTH_DEFICIT_INPUT);
        // Deficit 0.55 — well above midpoint+0.1; axis saturates.
        assert!(
            c.evaluate(0.55) > 0.80,
            "injury curve at deficit=0.55 must saturate (>0.80), got {}",
            c.evaluate(0.55)
        );
        // Deficit 1.0 — full HP loss; axis ≈1.0.
        assert!(
            c.evaluate(1.0) > 0.99,
            "injury curve at deficit=1.0 must be ≈1.0, got {}",
            c.evaluate(1.0)
        );
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

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static SLEEP_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 200,
        construct: |s| sleep_dse(s),
    };
