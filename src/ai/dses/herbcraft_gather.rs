//! `Herbcraft::GatherHerbs` — sibling-DSE split from the retiring
//! cat `Herbcraft` inline block (§L2.10.10).
//!
//! `CompensatedProduct` of spirituality + herbcraft_skill +
//! territory_max_corruption. All three gate — collecting herbs is a
//! devotional/craft activity; neither pure spirituality nor pure
//! skill suffices alone, and the corruption axis only activates the
//! surge when territory corruption is actually present. Eligibility:
//! `has_herbs_nearby` (outer gate). Maslow tier 2.
//!
//! The `territory_max_corruption` axis uses the §2.3 Logistic(8, 0.1)
//! shape — threshold-gated surge that rises steeply past 0.1
//! corruption. Absorbs the retiring
//! `ward_corruption_emergency_bonus` modifier contribution: the old
//! flat additive bonus-when-corruption-detected is now produced by
//! the axis curve itself as a natural threshold response, consistent
//! with §2.3's retirement unify-shape pattern ("each retired constant
//! was a flat additive bonus gated by a compound threshold, used to
//! overcome the fact that the underlying axis was being scored
//! linearly").

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

pub const SPIRITUALITY_INPUT: &str = "spirituality";
pub const HERBCRAFT_SKILL_INPUT: &str = "herbcraft_skill";

/// §L2.10.7 HerbcraftGather range — Manhattan tiles for the
/// nearest-herb-patch anchor. 20 ≈ a routine herb-gathering walk
/// (matches Cook/Eat/Build commute scale).
pub const HERBCRAFT_GATHER_PATCH_RANGE: f32 = 20.0;

pub struct HerbcraftGatherDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HerbcraftGatherDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // §L2.10.7 row Herbcraft (Gather): Composite{Logistic(8, 0.5),
        // Invert} over distance to nearest harvestable herb patch.
        // Spec line 5635: 'Herb commute; emergency-corruption boost
        // handled by scalar, not spatial.' Replaces the retired
        // `territory_max_corruption` Logistic(8, 0.1) scalar — the
        // emergency-corruption signal now flows entirely through
        // ColonyCleanseDse / DurableWardDse via the territory and
        // hotspot anchors. Gathering is a routine errand whose pull
        // depends on patch proximity, not corruption. ClampMin(0.1)
        // floor so distant cats still contribute under CP.
        let patch_distance = Curve::Composite {
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
            id: DseId("herbcraft_gather"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SPIRITUALITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HERBCRAFT_SKILL_INPUT, linear)),
                Consideration::Spatial(SpatialConsideration::new(
                    "herbcraft_gather_patch_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestHerbPatch),
                    HERBCRAFT_GATHER_PATCH_RANGE,
                    patch_distance,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for HerbcraftGatherDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HerbcraftGatherDse {
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
                label: "herbs_in_inventory",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn herbcraft_gather_dse() -> Box<dyn Dse> {
    Box::new(HerbcraftGatherDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn herbcraft_gather_id_stable() {
        assert_eq!(HerbcraftGatherDse::new().id().0, "herbcraft_gather");
    }

    #[test]
    fn herbcraft_gather_has_three_axes() {
        // §L2.10.7: spirituality + herbcraft_skill + patch_distance.
        let dse = HerbcraftGatherDse::new();
        assert_eq!(dse.considerations().len(), 3);
    }

    #[test]
    fn herbcraft_gather_uses_herb_patch_anchor() {
        let dse = HerbcraftGatherDse::new();
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(s) if s.name == "herbcraft_gather_patch_distance" => {
                    Some(s)
                }
                _ => None,
            })
            .expect("herbcraft_gather_patch_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::NearestHerbPatch)
        ));
        // Composite{Composite{Logistic(8, 0.5), Invert}, ClampMin(0.1)}:
        // at cost 0 ≈ 0.98, midpoint 0.5 ≈ 0.5, edge 1.0 floored at 0.1
        // (raw Logistic-Invert would be ≈ 0.018; floor preserves
        // build-pressure feedback under CP composition).
        assert!(approx(spatial.curve.evaluate(0.0), 0.982, 1e-2));
        assert!(approx(spatial.curve.evaluate(0.5), 0.5, 1e-2));
        assert!(approx(spatial.curve.evaluate(1.0), 0.1, 1e-2));
    }
}
