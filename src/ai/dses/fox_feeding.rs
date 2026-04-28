//! Fox `Feeding` — not in any §3.3.2 peer group (offspring-care
//! action, Maslow tier 3 suppressed by survival × territory).
//!
//! Per §2.3 + §3.1.1 row 1532: `CompensatedProduct` of 2 axes —
//! `cub_satiation_deficit` via `Logistic(7, 0.6)` (cub-hunger
//! threshold; gentler than adult hangry at 8/0.75 because adults
//! buffer the gap) and `protectiveness` via Linear. Both gate.
//!
//! Eligibility: `has_cubs && cubs_hungry` (outer gate).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const CUB_SATIATION_DEFICIT_INPUT: &str = "cub_satiation_deficit";
pub const PROTECTIVENESS_INPUT: &str = "protectiveness";

/// §L2.10.7 fox Feeding range — Manhattan tiles for the
/// home-den anchor. 12 tiles ≈ a fox's territorial radius (matches
/// fox Resting). Power curve gives sharp 'return-to-den' pull.
pub const FOX_FEEDING_DEN_RANGE: f32 = 12.0;

pub struct FoxFeedingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxFeedingDse {
    pub fn new() -> Self {
        // §L2.10.7 row Feeding: Power curve over distance to den.
        // 'Return-to-den is highly localized' per spec line 5649.
        let den_distance = Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        };
        Self {
            id: DseId("fox_feeding"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    CUB_SATIATION_DEFICIT_INPUT,
                    Curve::Logistic {
                        steepness: 7.0,
                        midpoint: 0.6,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    PROTECTIVENESS_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Spatial(SpatialConsideration::new(
                    "fox_feeding_den_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::OwnDen),
                    FOX_FEEDING_DEN_RANGE,
                    den_distance,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for FoxFeedingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FoxFeedingDse {
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
                label: "cubs_fed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        3
    }
}

pub fn fox_feeding_dse() -> Box<dyn Dse> {
    Box::new(FoxFeedingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_feeding_id_stable() {
        assert_eq!(FoxFeedingDse::new().id().0, "fox_feeding");
    }

    #[test]
    fn fox_feeding_maslow_tier_is_three() {
        assert_eq!(FoxFeedingDse::new().maslow_tier(), 3);
    }

    #[test]
    fn fox_feeding_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FoxFeedingDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }
}
