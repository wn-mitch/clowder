//! `Farm` — Work-urgency peer (§3.3.2 anchor = 1.0). Also the
//! canonical "zero-to-nonzero" Phase 3 canary per the balance doc —
//! Farm must fire ≥ 1× on seed 42 to prove substrate dormancy (not
//! missing system) was the cause of its 0-fire baseline.
//!
//! Per §2.3 + §3.1.1 row 1494: `CompensatedProduct` of four axes —
//! `food_scarcity` via `scarcity()` (Quadratic(exp=2)), `diligence`
//! via Linear, `farm_garden_distance` spatial, and (ticket 084 Commit 3)
//! `farm_herb_pressure` via MarkerConsideration over the
//! `ColonyThornbriarChronicallyLow` chronicity marker. Commit 1's
//! Linear-on-0/1-scalar approximation has been retired — the chronicity
//! marker carries the same 0/1 shape but reflects sustained stash
//! depletion across `chronicity_window_ticks` rather than transient
//! per-tick state. Mirrors `BuildDse`'s `ColonyStoresChronicallyFull`
//! axis (ticket 179).
//!
//! Eligibility: `.require("HasGarden")` per §4 port (Phase 4b.4).
//! Maslow tier 2.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, MarkerConsideration, ScalarConsideration,
    SpatialConsideration,
};
use crate::ai::curves::{scarcity, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub const FOOD_SCARCITY_INPUT: &str = "food_scarcity";
pub const DILIGENCE_INPUT: &str = "diligence";
/// Ticket 084 Commit 3 — herb/ward demand axis (chronicity-marker
/// flavor). Now a `MarkerConsideration` over
/// `ColonyThornbriarChronicallyLow` rather than the prior 0/1 scalar
/// sourced from `ctx_scalars`. The marker latches at
/// `chronicity_window_ticks` boundaries against the colony-wide
/// thornbriar stash sum; firing it lifts Farm's CompensatedProduct
/// out of the food-stockpile-full trap by giving Farm a non-food
/// demand axis.
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
    pub fn new(scoring: &ScoringConstants) -> Self {
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
                // 084 Commit 3 — herb/ward demand axis (chronicity
                // marker flavor). Mirrors `BuildDse`'s
                // `ColonyStoresChronicallyFull` axis. Firing this
                // marker lifts Farm's CompensatedProduct out of the
                // food-stockpile-full trap, pairing the DSE's
                // motivation with the coordinator's garden-repurposing
                // decision so a Thornbriar plot draws a farmer instead
                // of sitting at growth = 0.
                Consideration::Marker(MarkerConsideration::new(
                    FARM_HERB_PRESSURE_INPUT,
                    markers::ColonyThornbriarChronicallyLow::KEY,
                    scoring.farm_herb_pressure_weight,
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
    /// Default uses `ScoringConstants::default()` — convenience for
    /// tests; production routes through `farm_dse(scoring)`.
    fn default() -> Self {
        Self::new(&ScoringConstants::default())
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
impl crate::ai::dse::CatDse for FarmDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Farm
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn farm_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(FarmDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farm_dse_id_stable() {
        assert_eq!(FarmDse::default().id().0, "farm");
    }

    #[test]
    fn farm_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FarmDse::default().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn farm_dse_has_herb_pressure_axis() {
        // 084 Commit 3 — Farm's herb-pressure axis migrated from
        // ScalarConsideration to MarkerConsideration over the
        // ColonyThornbriarChronicallyLow chronicity marker. The fourth
        // axis is now a Marker rather than a Scalar.
        let dse = FarmDse::default();
        let herb_axis = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Marker(m) if m.name == FARM_HERB_PRESSURE_INPUT => Some(m),
                _ => None,
            })
            .expect("FarmDse must include the herb-pressure MarkerConsideration");
        assert_eq!(
            herb_axis.marker,
            markers::ColonyThornbriarChronicallyLow::KEY
        );
        // Composition must carry one weight per consideration.
        assert_eq!(dse.composition().weights.len(), dse.considerations().len());
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static FARM_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 1600,
        construct: |s| farm_dse(s),
    };
