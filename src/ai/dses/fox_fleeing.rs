//! Fox `Fleeing` — Fatal-threat peer (§3.3.2 anchor = 1.0). Peer of
//! cat `Flee` + `Fight` and fox `Avoiding` + `DenDefense`.
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of three axes — `health_deficit`
//! via `Logistic(8, 0.5)` (injury-panic threshold), `cats_nearby` via
//! `Piecewise` step at 2+, `boldness` via `Composite { Linear(slope=
//! 0.5), Invert }` (damped invert — timid foxes flee more, but
//! boldness is a modulator, not a gate).
//!
//! Maslow tier 1 — same as fox Hunting/Raiding (survival).
//!
//! **Shape vs. inline.** Old formula:
//! `((1 - health) + cats_bonus) × (1 - bold × 0.5) × l1` with no
//! ceiling. Peak above 1.0 when `cats_nearby ≥ 2` and injured + timid.
//! Port compresses to 1.0 under RtEO so Fleeing sits at peer-group
//! magnitude.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{piecewise, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";
pub const CATS_NEARBY_INPUT: &str = "cats_nearby";
pub const BOLDNESS_INPUT: &str = "boldness";

pub struct FoxFleeingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxFleeingDse {
    pub fn new() -> Self {
        // Health-deficit Logistic: inflection at 0.5 matches the old
        // hardcoded `health_fraction < 0.5` gate.
        let health_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        // §2.3: step at 2+ cats. Piecewise knots (0,0),(1,0),(2,0.5),
        // (10,0.5) — cats_nearby = 0 or 1 → 0; 2+ → 0.5. This is a
        // *bonus*, not a proportional signal, so caps at 0.5.
        let cats_curve = piecewise(vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.5), (10.0, 0.5)]);
        // Damped invert: Linear(slope=0.5) maps boldness=1.0 → 0.5,
        // then Invert gives (1 - 0.5) = 0.5. Max-bold fox still
        // contributes 0.5; timid fox (bold=0) contributes 1.0.
        let boldness_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 0.5,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("fox_fleeing"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, health_curve)),
                Consideration::Scalar(ScalarConsideration::new(CATS_NEARBY_INPUT, cats_curve)),
                Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_curve)),
            ],
            // RtEO sum = 1.0. Health deficit dominates (panic when
            // injured). Cats-nearby is an escalation bonus. Boldness
            // damped-invert is personality modulation.
            composition: Composition::weighted_sum(vec![0.45, 0.25, 0.30]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for FoxFleeingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FoxFleeingDse {
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
                label: "fox_fled_to_safety",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn fox_fleeing_dse() -> Box<dyn Dse> {
    Box::new(FoxFleeingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_fleeing_id_stable() {
        assert_eq!(FoxFleeingDse::new().id().0, "fox_fleeing");
    }

    #[test]
    fn fox_fleeing_has_three_axes() {
        assert_eq!(FoxFleeingDse::new().considerations().len(), 3);
    }

    #[test]
    fn fox_fleeing_weights_sum_to_one() {
        let sum: f32 = FoxFleeingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn fox_fleeing_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FoxFleeingDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn fox_fleeing_maslow_tier_is_one() {
        assert_eq!(FoxFleeingDse::new().maslow_tier(), 1);
    }

    #[test]
    fn cats_nearby_steps_at_two() {
        let dse = FoxFleeingDse::new();
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Piecewise: (0,0),(1,0),(2,0.5),(10,0.5).
        assert!((c.evaluate(0.0) - 0.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.0).abs() < 1e-4);
        assert!((c.evaluate(2.0) - 0.5).abs() < 1e-4);
        assert!((c.evaluate(5.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn boldness_damped_invert() {
        let dse = FoxFleeingDse::new();
        let c = match &dse.considerations()[2] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Linear(slope=0.5) then Invert. boldness=0 → inner=0 → invert=1.
        // boldness=1 → inner=0.5 → invert=0.5.
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.5).abs() < 1e-4);
    }
}
