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
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{piecewise, sleep_dep, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

pub const ENERGY_DEFICIT_INPUT: &str = "energy_deficit";
pub const DAY_PHASE_INPUT: &str = "day_phase";
pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";

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

        Self {
            id: DseId("sleep"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(ENERGY_DEFICIT_INPUT, sleep_dep())),
                Consideration::Scalar(ScalarConsideration::new(DAY_PHASE_INPUT, day_phase_curve)),
                Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, injury_curve)),
            ],
            // RtEO sum = 1.0. Energy deficit dominates (the core
            // driver); day_phase carries the circadian rhythm;
            // injury_rest is a recovery modulator.
            composition: Composition::weighted_sum(vec![0.5, 0.3, 0.2]),
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
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "energy_restored",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
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
    fn sleep_has_three_axes() {
        let s = ScoringConstants::default();
        assert_eq!(SleepDse::new(&s).considerations().len(), 3);
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

    #[test]
    fn injury_curve_zero_at_full_health() {
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = match &dse.considerations()[2] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // health_deficit = 0 (full health) → Linear output = 0.
        assert!((c.evaluate(0.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn day_phase_knots_match_scoring_constants() {
        let s = ScoringConstants::default();
        let dse = SleepDse::new(&s);
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!((c.evaluate(DAWN_KNOT) - s.sleep_dawn_bonus).abs() < 1e-4);
        assert!((c.evaluate(NIGHT_KNOT) - s.sleep_night_bonus).abs() < 1e-4);
    }
}
