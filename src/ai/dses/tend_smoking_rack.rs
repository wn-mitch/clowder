//! `TendSmokingRack` — Phase 1b auxiliary DSE (ticket 367).
//!
//! Sibling to `SmokeMeatDse` — the smoking pipeline is two-phase: load
//! the rack (one resolver call, consumes meat + fuel from inventory),
//! then tend it across multiple cycles to advance progress.
//!
//! Per-tend mechanics: each tend advances
//! `SmokingRackState.progress` by `1.0 / tends_needed`, sets
//! `last_tended_at_tick = current_tick`. The colony-scoped
//! `HasLoadedSmokingRackOffCooldown` marker fires only when a loaded
//! rack with `progress < 1.0` has been silent for at least
//! `CraftingConstants::smoking_tend_cooldown_ticks`. The per-rack
//! cooldown is what forces the interleaving — a cat finishes one
//! tend, the rack drops off the eligibility set for ~2 sim-hours, the
//! cat picks something else (hunt / forage / socialise), then
//! eventually the rack re-enters the eligibility pool.
//!
//! No `HasSmokeableInInventory` gate — tending doesn't consume
//! inventory. The cat just needs to be near the rack.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const TEND_SMOKING_RACK_RANGE: f32 = 20.0;

pub struct TendSmokingRackDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl TendSmokingRackDse {
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
            id: DseId("tend_smoking_rack"),
            considerations: vec![
                // base_rate — tending is intrinsically motivating when
                // a rack is sitting off-cooldown. The
                // `HasLoadedSmokingRackOffCooldown` marker carries the
                // "rack is ready" signal; the DSE doesn't need a
                // separate scalar urgency on top.
                Consideration::Scalar(ScalarConsideration::new(
                    "one",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Spatial(SpatialConsideration::new(
                    "tend_smoking_rack_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestKitchen),
                    TEND_SMOKING_RACK_RANGE,
                    rack_distance,
                )),
            ],
            // Three-axis composition: base / diligence / distance. No
            // scarcity term — tending is reactive to a loaded rack,
            // not driven by food-buffer pressure. Three equal-ish
            // weights summing to 1.0.
            composition: Composition::weighted_sum(vec![0.40, 0.30, 0.30]),
            eligibility: EligibilityFilter::new()
                .require(markers::CanSmoke::KEY)
                .require(markers::HasLoadedSmokingRackOffCooldown::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for TendSmokingRackDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for TendSmokingRackDse {
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
                label: "smoking_rack_tended",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}
impl crate::ai::dse::CatDse for TendSmokingRackDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::TendSmokingRack
    }
}


pub fn tend_smoking_rack_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(TendSmokingRackDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tend_smoking_rack_dse_id_stable() {
        assert_eq!(TendSmokingRackDse::new().id().0, "tend_smoking_rack");
    }

    #[test]
    fn tend_smoking_rack_weights_sum_to_one() {
        let dse = TendSmokingRackDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn tend_smoking_rack_dse_eligibility_shape() {
        let dse = TendSmokingRackDse::new();
        // The cooldown-aware HasLoadedSmokingRackOffCooldown marker is
        // load-bearing — without it, the DSE would re-fire every tick
        // on the same rack regardless of cooldown.
        assert_eq!(
            dse.eligibility().required,
            vec![
                markers::CanSmoke::KEY,
                markers::HasLoadedSmokingRackOffCooldown::KEY,
            ]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static TEND_SMOKING_RACK_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3200,
        construct: |_| tend_smoking_rack_dse(),
    };
