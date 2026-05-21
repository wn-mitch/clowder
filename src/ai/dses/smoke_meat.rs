//! `SmokeMeat` — Phase 1b preservation (ticket 367).
//!
//! Loads the Smoking Rack with raw meat + fuel. Smoking is then driven
//! by discrete tend-cycles via the sibling `TendSmokingRackDse` —
//! smoking progress doesn't tick continuously, it advances 1/N per
//! tend with a per-rack cooldown between visits.
//!
//! See `dry_food.rs` for the broader composition / weight discussion;
//! this DSE mirrors it, swapping the disjunction inventory marker for
//! the conjunction marker `HasSmokeableInInventory` (meat AND fuel).

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

pub const SMOKE_MEAT_RACK_RANGE: f32 = 20.0;

pub struct SmokeMeatDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SmokeMeatDse {
    pub fn new() -> Self {
        let rack_distance = Curve::Composite {
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
            id: DseId("smoke_meat"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    "one",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                // Ticket 439 retired the `NearestKitchen` Commit-4
                // placeholder for `NearestSmokingRack` — populated
                // per-cat from `CatAnchorPositions.nearest_smoking_rack`
                // (which reads the same `smoking_rack_positions` slice
                // the planner zone resolver consumes at
                // `systems/goap.rs:1739`).
                Consideration::Spatial(SpatialConsideration::new(
                    "smoke_meat_rack_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestSmokingRack),
                    SMOKE_MEAT_RACK_RANGE,
                    rack_distance,
                )),
            ],
            composition: Composition::weighted_sum(vec![0.32, 0.24, 0.24, 0.20]),
            eligibility: EligibilityFilter::new()
                .require(markers::CanSmoke::KEY)
                .require(markers::HasFunctionalSmokingRack::KEY)
                // 443: widened from `HasSmokeableInInventory` to
                // `HasSmokeableAccessible` — fires when the cat
                // already carries smokeable items OR has a free slot
                // and the colony's stores hold raw meat + fuel.
                // Without this, cats deposit at Stores on hunt-return
                // and the DSE is permanently ineligible despite full
                // stores.
                .require(markers::HasSmokeableAccessible::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for SmokeMeatDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for SmokeMeatDse {
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
                label: "meat_loaded_on_smoking_rack",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}
impl crate::ai::dse::CatDse for SmokeMeatDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::SmokeMeat
    }
}

pub fn smoke_meat_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(SmokeMeatDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_meat_dse_id_stable() {
        assert_eq!(SmokeMeatDse::new().id().0, "smoke_meat");
    }

    #[test]
    fn smoke_meat_weights_sum_to_one() {
        let dse = SmokeMeatDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn smoke_meat_dse_eligibility_shape() {
        let dse = SmokeMeatDse::new();
        assert_eq!(
            dse.eligibility().required,
            vec![
                markers::CanSmoke::KEY,
                markers::HasFunctionalSmokingRack::KEY,
                markers::HasSmokeableAccessible::KEY,
            ]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static SMOKE_MEAT_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3100,
        construct: |_| smoke_meat_dse(),
    };
