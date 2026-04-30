//! `Farm` — Work-urgency peer (§3.3.2 anchor = 1.0). Also the
//! canonical "zero-to-nonzero" Phase 3 canary per the balance doc —
//! Farm must fire ≥ 1× on seed 42 to prove substrate dormancy (not
//! missing system) was the cause of its 0-fire baseline.
//!
//! Per §2.3 + §3.1.1 row 1494: `CompensatedProduct` of four axes —
//! `food_scarcity` via `scarcity()` (Quadratic(exp=2)), `diligence`
//! via Linear, `farm_garden_distance` spatial, and (ticket 084)
//! `farm_herb_pressure` via Linear identity over a 0/1 scalar that
//! mirrors the `ward_strength_low && !thornbriar_available`
//! condition the coordinator uses to repurpose a FoodCrops garden
//! into a Thornbriar plot. The herb-pressure axis is the demand
//! signal that lets a Thornbriar plot draw a farmer when food
//! stockpiles are full but ward stockpile is empty — without it,
//! Farm scored to ~0 via `food_scarcity` and the repurposed plot
//! sat at growth = 0.
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
/// Ticket 084 — herb/ward demand axis. Scalar is 1.0 when
/// `ward_strength_low && !thornbriar_available` (the same condition
/// `coordination.rs::evaluate_coordinators` uses to repurpose a
/// FoodCrops garden to Thornbriar), 0.0 otherwise. Sourced from
/// `ctx_scalars` in `scoring.rs`.
pub const FARM_HERB_PRESSURE_INPUT: &str = "farm_herb_pressure";

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
        // ColonyLandmarks. Composite{Logistic, Invert} shape for the
        // close-enough plateau; ClampMin(0.1) outer floor so distant
        // cats still score non-zero. Without the floor, CP gates
        // Farm to 0 for any cat outside ~12 tiles, which broke the
        // build-pressure → garden-built feedback loop in the
        // closeout soak (CropTended/CropHarvested stopped firing).
        // The spec's 'high-cost candidates degrade smoothly' wording
        // (considerations.rs:73) reads "discount, not gate" — the
        // CanForage / HasGarden / HasFunctionalKitchen marker
        // eligibility filters still gate DSEs entirely when the
        // landmark doesn't exist.
        let garden_distance = Curve::Composite {
            inner: Box::new(Curve::Composite {
                inner: Box::new(Curve::Logistic {
                    steepness: 8.0,
                    midpoint: 0.5,
                }),
                post: PostOp::Invert,
            }),
            post: PostOp::ClampMin(0.1),
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
                // Ticket 084 — herb/ward demand axis. Linear identity
                // over a 0/1 scalar; CP compensation lifts Farm above
                // zero when this axis fires (1.0) even if
                // `food_scarcity` is 0 (food stockpile full). Pairs
                // the DSE's motivation with the coordinator's garden-
                // repurposing decision so a Thornbriar plot draws a
                // farmer instead of sitting at growth = 0.
                Consideration::Scalar(ScalarConsideration::new(
                    FARM_HERB_PRESSURE_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0, 1.0]),
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

    #[test]
    fn farm_dse_has_herb_pressure_axis() {
        // Ticket 084 — Farm carries a fourth axis tied to ward/herb
        // demand so a Thornbriar-repurposed garden draws a farmer
        // even with food stockpiles full.
        let dse = FarmDse::new();
        let inputs: Vec<&str> = dse
            .considerations()
            .iter()
            .filter_map(|c| match c {
                Consideration::Scalar(s) => Some(s.name),
                _ => None,
            })
            .collect();
        assert!(
            inputs.contains(&FARM_HERB_PRESSURE_INPUT),
            "FarmDse must read `farm_herb_pressure`; found scalar inputs: {inputs:?}"
        );
        // Composition must carry one weight per consideration.
        assert_eq!(dse.composition().weights.len(), dse.considerations().len());
    }
}
