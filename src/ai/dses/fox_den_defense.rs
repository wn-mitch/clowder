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
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{flee_or_fight, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const CUB_SAFETY_DEFICIT_INPUT: &str = "cub_safety_deficit";
pub const PROTECTIVENESS_INPUT: &str = "protectiveness";

/// §L2.10.7 fox DenDefense range — Manhattan tiles for the
/// home-den anchor. 8 tiles ≈ tighter than Resting/Feeding (12)
/// because DenDefense is an at-the-den action; foxes far from the
/// den can't defend even if they want to. Sharper Power fall-off.
pub const FOX_DEN_DEFENSE_DEN_RANGE: f32 = 8.0;

pub struct FoxDenDefenseDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxDenDefenseDse {
    pub fn new() -> Self {
        // §L2.10.7 row DenDefense: Power-Invert curve over distance
        // to den. Spec line 5652: 'Inverse-distance-from-den; sharper
        // than Flee because cubs anchor commitment.' Closer-to-den =
        // higher score, encoding 'stay near the cubs'.
        let den_distance = Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        };
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
                Consideration::Spatial(SpatialConsideration::new(
                    "fox_den_defense_den_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::OwnDen),
                    FOX_DEN_DEFENSE_DEN_RANGE,
                    den_distance,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
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
    fn fox_den_defense_has_three_axes() {
        // §L2.10.7: cub_safety_deficit + protectiveness + den_distance.
        assert_eq!(FoxDenDefenseDse::new().considerations().len(), 3);
    }

    #[test]
    fn fox_den_defense_uses_own_den_anchor() {
        let dse = FoxDenDefenseDse::new();
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "fox_den_defense_den_distance" => {
                    Some(sp)
                }
                _ => None,
            })
            .expect("fox_den_defense_den_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::OwnDen)
        ));
        // Power-Invert: closer = higher.
        assert!(spatial.curve.evaluate(0.0) > 0.99);
        assert!(spatial.curve.evaluate(1.0) < 0.01);
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
