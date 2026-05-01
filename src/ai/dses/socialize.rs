//! `Socialize` — Social-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of 8 axes — the highest-n
//! RtEO DSE in the catalog aside from Fight. Loneliness-anchor
//! Logistic on social_deficit; sociability + temper + playfulness
//! personality coefficients; inverted-need-penalty on phys_sat
//! (bilinear temper × (1 - phys_sat) lives in composition); and a
//! threshold-gated corruption pushback bonus. Ticket 122 added a
//! satiation axis on raw `social` so the producer mirrors the §7.2
//! OpenMinded gate's `still_goal` predicate
//! (`needs.social < social_satiation_threshold`); without it,
//! well-bonded cats elected Socialize and the gate dropped the plan
//! same-tick (588× in seed-42's cold-start window).
//!
//! Maslow tier 2 — matches the old inline `level_suppression(2)`.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{inverted_need_penalty, loneliness, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const SOCIAL_DEFICIT_INPUT: &str = "social_deficit";
pub const SOCIABILITY_INPUT: &str = "sociability";
pub const TEMPER_INPUT: &str = "temper";
pub const PHYS_SATISFACTION_INPUT: &str = "phys_satisfaction";
pub const PLAYFULNESS_INPUT: &str = "playfulness";
pub const TILE_CORRUPTION_INPUT: &str = "tile_corruption";
pub const SOCIAL_WARMTH_DEFICIT_INPUT: &str = "social_warmth_deficit";
/// Ticket 122 — IAUS-side mirror of the §7.2 OpenMinded gate's
/// `still_goal` predicate. Reads raw `social` need; the curve
/// suppresses the score above `social_satiation_threshold` so the
/// producer doesn't elect Socialize for plans the gate would drop.
pub const SOCIAL_SATIATION_INPUT: &str = "social_satiation";

pub struct SocializeDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SocializeDse {
    pub fn new() -> Self {
        // Corruption bonus: threshold gate at 0.1 absorbed into the
        // curve per §2.3 row 1025. Logistic(8, 0.1) maps
        // tile_corruption < 0.1 to near-0 and above to positive.
        let corruption_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.1,
        };
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };

        // Ticket 122 — satiation axis. Logistic(8, 0.85) centred on
        // `social_satiation_threshold = 0.85`; the Invert post-op
        // gives `social = 0` → ~1 (no penalty) and `social = 1` →
        // ~0 (fully suppressed). Steepness 8 (vs the 5 used in
        // `inverted_need_penalty()`) gives a sharper drop at the
        // threshold so the axis bites visibly around 0.85, matching
        // the gate's hard cutoff in shape if not in value. Stays
        // additive (not multiplicative) — the §"Out of scope" note
        // on ticket 122 cautions against turning this into an
        // eligibility filter.
        let social_satiation_curve = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.85,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("socialize"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SOCIAL_DEFICIT_INPUT, loneliness())),
                Consideration::Scalar(ScalarConsideration::new(SOCIABILITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(TEMPER_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(
                    PHYS_SATISFACTION_INPUT,
                    inverted_need_penalty(),
                )),
                Consideration::Scalar(ScalarConsideration::new(PLAYFULNESS_INPUT, linear)),
                Consideration::Scalar(ScalarConsideration::new(
                    TILE_CORRUPTION_INPUT,
                    corruption_curve,
                )),
                // §7.W: social_warmth fulfillment deficit. Small
                // additive rider — socializing addresses social_warmth
                // at a slower rate than grooming, so the weight is
                // lower.
                Consideration::Scalar(ScalarConsideration::new(
                    SOCIAL_WARMTH_DEFICIT_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.1,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    SOCIAL_SATIATION_INPUT,
                    social_satiation_curve,
                )),
            ],
            // RtEO sum = 1.0. Loneliness dominates; sociability +
            // playfulness are secondary personality drivers. Temper
            // and phys_sat inverted-penalty jointly express the old
            // `temper × (1-phys_sat)` subtraction as two positive
            // additive axes (both high ⇒ strong modulation of the
            // score downward via the non-social axes dominating).
            // Corruption bonus is a small-weight additive rider.
            // Social-warmth deficit (0.08) is a gentle nudge.
            // Ticket 122 — satiation axis at weight 0.30. The
            // existing seven weights renormalize ×0.70 so the sum
            // stays ≈ 1.0; at full satiation (signal ≈ 0), the
            // weighted-sum drops by 30% from pre-fix baseline,
            // empirically enough to push Socialize below other tier-2
            // DSEs without zeroing the score (a hard zero would
            // over-correct against the §"Out of scope" caution).
            composition: Composition::weighted_sum(vec![
                0.32 * 0.70,
                0.19 * 0.70,
                0.05 * 0.70,
                0.09 * 0.70,
                0.19 * 0.70,
                0.08 * 0.70,
                0.08 * 0.70,
                0.30,
            ]),
            // §13.1: `.forbid(markers::Incapacitated::KEY)` blocks downed cats.
            // §9.3 stance binding migrated to `socialize_target_dse` —
            // candidate-prefilter happens in the target-taking resolver,
            // not on the cat-action DSE which has no candidate set.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for SocializeDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for SocializeDse {
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
        // §7.3: Socializing → OpenMinded. Activity-shaped; drops on
        // sated-sociability or lost interest.
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "socialized",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn socialize_dse() -> Box<dyn Dse> {
    Box::new(SocializeDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socialize_dse_id_stable() {
        assert_eq!(SocializeDse::new().id().0, "socialize");
    }

    #[test]
    fn socialize_has_eight_axes() {
        // Ticket 122 added the social_satiation axis (was 7).
        assert_eq!(SocializeDse::new().considerations().len(), 8);
    }

    #[test]
    fn socialize_weights_sum_to_one() {
        let sum: f32 = SocializeDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn socialize_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            SocializeDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn socialize_maslow_tier_is_two() {
        assert_eq!(SocializeDse::new().maslow_tier(), 2);
    }

    #[test]
    fn socialize_satiation_axis_is_present_and_last() {
        // Ticket 122 — the satiation axis is the new 8th consideration.
        // Its position matters for the composition weights vector
        // alignment (weights[7] = 0.30 must pair with this axis).
        let dse = SocializeDse::new();
        let last = dse.considerations().last().expect("at least one axis");
        match last {
            Consideration::Scalar(s) => {
                assert_eq!(s.name, SOCIAL_SATIATION_INPUT);
            }
            _ => panic!("expected Scalar consideration on satiation axis"),
        }
        assert!(
            (dse.composition().weights[7] - 0.30).abs() < 1e-4,
            "satiation axis weight must be 0.30; got {}",
            dse.composition().weights[7]
        );
    }

    #[test]
    fn socialize_satiation_curve_suppresses_above_threshold() {
        // Ticket 122 — the curve must read near 0 at full satiation
        // and near 1 at zero satiation, with the steepness-8 logistic
        // crossing ≈ 0.5 at the gate threshold (0.85). Without these
        // monotonicity properties the producer cannot mirror the
        // gate's `still_goal` predicate.
        let dse = SocializeDse::new();
        let satiation = match dse.considerations().last().unwrap() {
            Consideration::Scalar(s) => s,
            _ => panic!("expected Scalar consideration"),
        };
        let unsated = satiation.score(0.0);
        let at_threshold = satiation.score(0.85);
        let sated = satiation.score(1.0);

        assert!(
            unsated > 0.95,
            "unsated cat should score ~1.0 (no penalty); got {unsated}"
        );
        assert!(
            sated < 0.30,
            "fully-sated cat should score near 0; got {sated}"
        );
        assert!(
            at_threshold < unsated && at_threshold > sated,
            "threshold score must sit between sated and unsated; got {at_threshold}"
        );
        assert!(
            (at_threshold - 0.5).abs() < 0.10,
            "logistic midpoint at 0.85 should give ~0.5 at the threshold; got {at_threshold}"
        );
    }
}
