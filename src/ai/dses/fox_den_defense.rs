//! Fox `DenDefense` — Fatal-threat peer (§3.3.2 anchor = 1.0).
//! Absolute-anchor peer of cat `Flee` through the shared
//! flee-or-fight Logistic shape (steepness=10, midpoint=0.5).
//!
//! Per §2.3 + §3.1.1: `CompensatedProduct` of two axes —
//! `cub_safety_deficit` via `flee_or_fight(0.5)` and `protectiveness`
//! via `Linear`. Both gate: no cub threat ⇒ no defense; no
//! protectiveness (drop-my-cubs fox) ⇒ no defense.
//!
//! Maslow tier 3 — matches the inline `l3` (offspring-layer)
//! suppression. DenDefense is only pursued when survival + territory
//! levels are satisfied enough.
//!
//! **Shape vs. inline.** Old formula:
//! `(1 - cub_safety) × protectiveness × 2.0 × l3`. The `× 2.0`
//! amplifier pushed peak above 1.0. Port uses the named
//! flee-or-fight anchor (steepness=10 Logistic) which saturates at
//! ~0.99, and CP with RtM weights [1.0, 1.0] keeps the peak bounded
//! by the peer-group 1.0 ceiling.
//!
//! **Eligibility gate.** `cat_threatening_den && has_cubs` stays
//! outer in `score_fox_dispositions`.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{flee_or_fight, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const CUB_SAFETY_DEFICIT_INPUT: &str = "cub_safety_deficit";
pub const PROTECTIVENESS_INPUT: &str = "protectiveness";

pub struct FoxDenDefenseDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxDenDefenseDse {
    pub fn new() -> Self {
        Self {
            id: DseId("fox_den_defense"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    CUB_SAFETY_DEFICIT_INPUT,
                    flee_or_fight(0.5),
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    PROTECTIVENESS_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for FoxDenDefenseDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FoxDenDefenseDse {
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
                label: "den_defended",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        3
    }
}

pub fn fox_den_defense_dse() -> Box<dyn Dse> {
    Box::new(FoxDenDefenseDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_den_defense_id_stable() {
        assert_eq!(FoxDenDefenseDse::new().id().0, "fox_den_defense");
    }

    #[test]
    fn fox_den_defense_has_two_axes() {
        assert_eq!(FoxDenDefenseDse::new().considerations().len(), 2);
    }

    #[test]
    fn fox_den_defense_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FoxDenDefenseDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn fox_den_defense_maslow_tier_is_three() {
        assert_eq!(FoxDenDefenseDse::new().maslow_tier(), 3);
    }

    #[test]
    fn safe_cubs_zero_score() {
        let dse = FoxDenDefenseDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // flee_or_fight(0.5): steep Logistic, near-zero below 0.3.
        // cub_safety_deficit = 0 means cubs fully safe → near-zero
        // defense score.
        assert!(c.evaluate(0.0) < 0.01);
    }

    #[test]
    fn threatened_cubs_saturate() {
        let dse = FoxDenDefenseDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Deficit = 0.9 → saturates near 1.0.
        assert!(c.evaluate(0.9) > 0.95);
    }
}
