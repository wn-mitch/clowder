//! `Idle` — Rest-urgency peer (§3.3.2 anchor = 1.0), but by design
//! "caps below Sleep's peak" (§3.3.2 row note). The always-available
//! low-floor fallback — when nothing else scores, the cat idles.
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of three axes — `base_rate`,
//! `incuriosity`, `playfulness` (inverted as a penalty). Floor is a
//! post-composition concern per §3.1.1 row 1510 ("Floor is a
//! post-composition `Clamp(min)`, not an axis"); in practice the
//! base_rate axis's Linear intercept carries the floor. A dedicated
//! `IdleFloor` modifier can land alongside the §3.5 modifier catalog
//! in a later phase if explicit post-composition flooring becomes
//! worth the ceremony.
//!
//! **Magnitude anchor.** Old inline: `idle_base(0.05) + (1-curiosity)
//! × 0.08 − playfulness × 0.05`, floored at `idle_minimum_floor =
//! 0.01`. Peak ~0.13. Ported WS with axis outputs scaled by the
//! original constants (slopes/intercepts) preserves those per-axis
//! contributions — under RtEO, with weights summing to 1.0, the
//! composed score magnitude lands near the old additive total
//! rather than blowing up to the peer-group 1.0 ceiling.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    ActivityKind, CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention,
    Termination,
};
use crate::resources::sim_constants::ScoringConstants;

pub const ONE_INPUT: &str = "one";
pub const INCURIOSITY_INPUT: &str = "incuriosity";
pub const PLAYFULNESS_INVERT_INPUT: &str = "playfulness_invert";

pub struct IdleDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl IdleDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // base_rate axis: constant `idle_base` regardless of input.
        // Slope 0 + intercept `idle_base` gives a flat axis output;
        // the caller feeds the shared "one" scalar for schema
        // uniformity. ClampMin floors the axis at
        // `idle_minimum_floor` — the post-composition floor per §2.3
        // baked into the base axis.
        let base_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 0.0,
                intercept: scoring.idle_base,
            }),
            post: crate::ai::curves::PostOp::ClampMin(scoring.idle_minimum_floor),
        };
        // Incuriosity axis: scaled Linear — `incuriosity_scale ×
        // incuriosity`. The scalar `incuriosity = 1 − curiosity` is
        // pre-computed in `ctx_scalars` so the curve stays a plain
        // Linear.
        let incuriosity_curve = Curve::Linear {
            slope: scoring.idle_incuriosity_scale,
            intercept: 0.0,
        };
        // Playfulness penalty: old formula subtracts
        // `playfulness × penalty`. Linear primitives clamp at 0, so
        // a negative slope can't represent the subtraction directly.
        // Instead, feed the inverted scalar `playfulness_invert =
        // 1 − playfulness` — a playful cat (scalar 0) contributes 0,
        // an unplayful cat (scalar 1) contributes the full penalty
        // magnitude. Preserves the "playful cats idle less" signal.
        let playfulness_curve = Curve::Linear {
            slope: scoring.idle_playfulness_penalty,
            intercept: 0.0,
        };

        Self {
            id: DseId("idle"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(ONE_INPUT, base_curve)),
                Consideration::Scalar(ScalarConsideration::new(
                    INCURIOSITY_INPUT,
                    incuriosity_curve,
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    PLAYFULNESS_INVERT_INPUT,
                    playfulness_curve,
                )),
            ],
            // RtEO weights. Close to even — the axis *outputs* already
            // encode the old formula's relative magnitudes, so weights
            // don't need to carry them a second time.
            composition: Composition::weighted_sum(vec![0.4, 0.35, 0.25]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for IdleDse {
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
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Idle,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        // Idle is the "always available" fallback — opt out of the
        // Maslow pre-gate so it keeps scoring regardless of tier.
        u8::MAX
    }
}

pub fn idle_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(IdleDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(IdleDse::new(&s).id().0, "idle");
    }

    #[test]
    fn idle_has_three_axes() {
        let s = ScoringConstants::default();
        assert_eq!(IdleDse::new(&s).considerations().len(), 3);
    }

    #[test]
    fn idle_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            IdleDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn idle_opts_out_of_maslow() {
        let s = ScoringConstants::default();
        assert_eq!(IdleDse::new(&s).maslow_tier(), u8::MAX);
    }

    #[test]
    fn base_axis_floors_at_idle_minimum() {
        let s = ScoringConstants::default();
        let dse = IdleDse::new(&s);
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // base_rate outputs max(idle_base, idle_minimum_floor) for any
        // input. Default constants put idle_base (0.05) above
        // idle_minimum_floor (0.01), so we should see idle_base.
        assert!((c.evaluate(1.0) - s.idle_base).abs() < 1e-4);
        // Even at input=0 the Linear slope=0 produces intercept
        // (idle_base), above the clamp floor.
        assert!((c.evaluate(0.0) - s.idle_base).abs() < 1e-4);
    }

    #[test]
    fn playful_cat_contributes_zero_on_playfulness_axis() {
        let s = ScoringConstants::default();
        let dse = IdleDse::new(&s);
        let c = match &dse.considerations()[2] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // playfulness_invert = 0 (fully playful) → axis output 0.
        assert!((c.evaluate(0.0) - 0.0).abs() < 1e-4);
    }
}
