//! `Farm` — Work-urgency peer (§3.3.2 anchor = 1.0). Also the
//! canonical "zero-to-nonzero" Phase 3 canary per the balance doc —
//! Farm must fire ≥ 1× on seed 42 to prove substrate dormancy (not
//! missing system) was the cause of its 0-fire baseline.
//!
//! Per §2.3 + §3.1.1 row 1494: `CompensatedProduct` of 2 axes —
//! `food_scarcity` via `scarcity()` (Quadratic(exp=2)) and
//! `diligence` via Linear. Both gate: no scarcity ⇒ no reason to
//! farm; no diligence ⇒ cat won't bother.
//!
//! Eligibility: `.require("HasGarden")` per §4 port (Phase 4b.4).
//! Maslow tier 2.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{scarcity, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const FOOD_SCARCITY_INPUT: &str = "food_scarcity";
pub const DILIGENCE_INPUT: &str = "diligence";

/// Manhattan range over which the garden-distance curve is normalized.
/// Same shape as Cook/Eat: 20 tiles.
pub const FARM_GARDEN_RANGE: f32 = 20.0;

pub struct FarmDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FarmDse {
    pub fn new() -> Self {
        // §L2.10.7 spatial axis: distance to garden tile via
        // ColonyLandmarks. Same Composite{Logistic, Invert} shape as
        // Cook — close-enough plateau, distant garden discounted.
        let garden_distance = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        Self {
            id: DseId("farm"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(FOOD_SCARCITY_INPUT, scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    DILIGENCE_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                // §L2.10.7 spatial axis. Multiplicative under
                // CompensatedProduct: distant garden discounts the
                // farm score. Marker eligibility (HasGarden) still
                // gates the DSE entirely when no garden exists.
                Consideration::Spatial(SpatialConsideration::new(
                    "farm_garden_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestGarden),
                    FARM_GARDEN_RANGE,
                    garden_distance,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §4 marker eligibility (Phase 4b.4): Farm only scores if
            // the colony has a functional garden. Retires the inline
            // `if ctx.has_garden` gate at `scoring.rs::score_actions`.
            // §13.1: `.forbid("Incapacitated")` blocks downed cats.
            eligibility: EligibilityFilter::new()
                .require(markers::HasGarden::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for FarmDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FarmDse {
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
                label: "farmed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn farm_dse() -> Box<dyn Dse> {
    Box::new(FarmDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farm_dse_id_stable() {
        assert_eq!(FarmDse::new().id().0, "farm");
    }

    #[test]
    fn farm_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FarmDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }
}
