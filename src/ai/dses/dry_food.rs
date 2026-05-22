//! `DryFood` — Phase 1b preservation (ticket 367).
//!
//! Cats load the Drying Rack with raw fish or raw organ (+ a herb for
//! the organ recipe). The rack runs the actual chemistry on its own —
//! progress advances per tick in `systems::preservation` only when
//! `weather.current == Weather::Clear`, so a cat can load the rack and
//! walk away. The DSE picks "is this worth doing right now?"; the
//! resolver and per-tick system handle "is it making progress yet?".
//!
//! Composition mirrors `CookDse`'s `WeightedSum { base_rate +
//! food_scarcity + diligence + station_distance }`. Preservation is
//! "always worth doing when food is plentiful" — the scarcity axis
//! reads inversely (high scarcity ⇒ less point preserving what little
//! you have, hunt fresh instead), but on stage-1 we use the same
//! `scarcity()` curve as Cook because the same intuition applies (cats
//! are food-buffer-building when scarcity is moderate; abandon the
//! buffer effort if hunger is acute).
//!
//! Per-cat eligibility (`CanDry`) + station availability
//! (`HasFunctionalDryingRack`) + inventory gate
//! (`HasDryableInInventory`) + Incapacitated forbid. The `HasDryable`
//! marker is a disjunction marker (fish OR organ) written by
//! `items::update_inventory_markers`; the resolver picks the specific
//! recipe at load time based on what's actually in the inventory.

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

/// Manhattan range over which the rack-distance curve normalises.
/// Reuses the Cook range — preservation stations colocate with food
/// infrastructure per `coordination::kind_affinity` (367-extended).
pub const DRY_FOOD_RACK_RANGE: f32 = 20.0;

pub struct DryFoodDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl DryFoodDse {
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
            id: DseId("dry_food"),
            considerations: vec![
                // base_rate — "drying is always worth something when
                // eligible." Dummy "one" scalar carries the magnitude
                // via the Linear slope.
                Consideration::Scalar(ScalarConsideration::new(
                    "one",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                // food_scarcity — preserves harder when colony food is
                // moderately scarce (a buffer-building motive); the
                // `scarcity()` curve naturally tapers at the extremes
                // (acute hunger should win Eat, abundant food doesn't
                // need preservation).
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                // Spatial axis: distance to nearest functional drying
                // rack. Ticket 439 retired the `NearestKitchen` Commit-4
                // placeholder in favor of a dedicated `NearestDryingRack`
                // anchor populated per-cat from
                // `CatAnchorPositions.nearest_drying_rack` (which reads
                // from the same `drying_rack_positions` slice the planner
                // zone resolver consumes — see
                // `systems/goap.rs:1733`). Co-lands with the
                // `building_snapshot` fix at `goap.rs:3728` that ensures
                // rack entities reach the snapshot at all.
                Consideration::Spatial(SpatialConsideration::new(
                    "dry_food_rack_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestDryingRack),
                    DRY_FOOD_RACK_RANGE,
                    rack_distance,
                )),
            ],
            // Same weight shape as Cook (0.32 / 0.24 / 0.24 / 0.20).
            // Spatial axis at 0.20 mirrors the §6.5 target-taking
            // precedent. Tuning lives in follow-on balance work after
            // Phase 1 hypothesis verifies.
            composition: Composition::weighted_sum(vec![0.32, 0.24, 0.24, 0.20]),
            // CanDry: Adult ∧ ¬Injured (capability-marker doctrine,
            // mirror of CookDse). HasFunctionalDryingRack +
            // HasDryableAccessible: station + accessibility (the
            // composite fires when the cat has dryable in inventory OR
            // a free slot AND the colony has dryable in stores; the
            // planner inserts a `RetrieveDryable` prefix in the latter
            // case). forbid Incapacitated: §13.1 — every
            // non-Eat/Sleep/Idle cat DSE forbids downed cats.
            //
            // Pre-follow-on the eligibility used the narrow
            // `HasDryableInInventory` and DryFood never fired on
            // seed-42 because cats deposit raw food at Stores
            // immediately on hunt-return, so the per-cat inventory
            // marker was off whenever scoring ran. See
            // `docs/open-work/tickets/367-…` Log entries.
            eligibility: EligibilityFilter::new()
                .require(markers::CanDry::KEY)
                .require(markers::HasFunctionalDryingRack::KEY)
                .require(markers::HasDryableAccessible::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for DryFoodDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for DryFoodDse {
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
                label: "food_loaded_on_drying_rack",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}
impl crate::ai::dse::CatDse for DryFoodDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::DryFood
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn dry_food_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(DryFoodDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_food_dse_id_stable() {
        assert_eq!(DryFoodDse::new().id().0, "dry_food");
    }

    #[test]
    fn dry_food_weights_sum_to_one() {
        let dse = DryFoodDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn dry_food_is_maslow_tier_2() {
        assert_eq!(DryFoodDse::new().maslow_tier(), 2);
    }

    #[test]
    fn dry_food_dse_eligibility_shape() {
        let dse = DryFoodDse::new();
        assert_eq!(
            dse.eligibility().required,
            vec![
                markers::CanDry::KEY,
                markers::HasFunctionalDryingRack::KEY,
                markers::HasDryableAccessible::KEY,
            ]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static DRY_FOOD_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3000,
        construct: |_| dry_food_dse(),
    };
