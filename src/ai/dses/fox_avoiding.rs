//! Fox `Avoiding` — Fatal-threat peer (§3.3.2 anchor = 1.0). The DSE
//! that dominated fox scoring after 3c.1b because its un-ported peak
//! exceeded the 1.0 anchor; porting it restores the peer-group
//! contract.
//!
//! Per §2.3 + §3.1.1: `CompensatedProduct` of two axes —
//! `cats_nearby` via a saturating-count `Composite` and `boldness`
//! via `Composite { Linear(slope=0.8), Invert }` (damped-invert,
//! stronger than `Fleeing`'s 0.5 damp). Damped-boldness is the
//! gate: max-bold fox never avoids.
//!
//! Maslow tier 1 — matches the inline `l1` factor.
//!
//! **Shape vs. inline.** Old formula: `cats_nearby × (1 - bold×0.8)
//! × l1`. No ceiling — a fox with 3 cats and boldness=0.2 sees
//! `3 × 0.84 × 1.0 = 2.52`, well above 1.0. That's what buried fox
//! Hunting in the 3c.1b soak. Porting to CP with the saturating
//! cats_nearby curve caps the peak near 0.88 (the `hangry()`
//! asymptote analog).
//!
//! **Cats-nearby saturation.** §2.3: "Reuse saturating-count
//! anchor." Used `Composite { Linear(slope=0.5), Clamp(max=1) }` so
//! saturation happens at cats_nearby = 2 — matches the old "1 cat =
//! weak pressure, 2+ cats = full pressure" intent. Using slope=1.0
//! would saturate at 1 cat and lose the 1-vs-2 discrimination
//! entirely.
//!
//! **Eligibility gate.** `cats_nearby ≥ 1 && hunger > 0.3 &&
//! health_fraction > 0.5` stays outer in `score_fox_dispositions`.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const CATS_NEARBY_INPUT: &str = "cats_nearby";
pub const BOLDNESS_INPUT: &str = "boldness";

pub struct FoxAvoidingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxAvoidingDse {
    pub fn new() -> Self {
        let cats_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 0.5,
                intercept: 0.0,
            }),
            post: PostOp::Clamp { min: 0.0, max: 1.0 },
        };
        let boldness_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 0.8,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("fox_avoiding"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(CATS_NEARBY_INPUT, cats_curve)),
                Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_curve)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for FoxAvoidingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FoxAvoidingDse {
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
                label: "cats_out_of_range",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn fox_avoiding_dse() -> Box<dyn Dse> {
    Box::new(FoxAvoidingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_avoiding_id_stable() {
        assert_eq!(FoxAvoidingDse::new().id().0, "fox_avoiding");
    }

    #[test]
    fn fox_avoiding_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FoxAvoidingDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn fox_avoiding_maslow_tier_is_one() {
        assert_eq!(FoxAvoidingDse::new().maslow_tier(), 1);
    }

    #[test]
    fn cats_curve_saturates_at_two() {
        let dse = FoxAvoidingDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Linear(0.5, 0) clamped to [0, 1]: 1 cat → 0.5, 2 cats → 1.0,
        // 5 cats → 1.0.
        assert!((c.evaluate(1.0) - 0.5).abs() < 1e-4);
        assert!((c.evaluate(2.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(5.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn max_bold_fox_never_avoids() {
        let dse = FoxAvoidingDse::new();
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Damped invert at slope 0.8: boldness=1 → inner=0.8 → invert=0.2.
        // boldness=0 → inner=0 → invert=1.0.
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.2).abs() < 1e-4);
    }
}
