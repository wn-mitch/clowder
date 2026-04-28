//! `Forage` — peer of Eat in the Starvation-urgency group.
//!
//! Per §2.3 + §3.1.1: WeightedSum of `hunger_urgency + food_scarcity
//! + diligence`.
//!
//! Design intent per §3.1.1: "A starving lazy cat should still
//! forage (desperation); a diligent cat should still forage when
//! colony stores are low."

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{hangry, scarcity, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

/// §L2.10.7 Forage range — Manhattan tiles for the
/// nearest-forageable-cluster anchor. 25 ≈ a routine errand walk;
/// matches Cook/Eat/Build commute scale.
pub const FORAGE_CLUSTER_RANGE: f32 = 25.0;

pub struct ForageDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ForageDse {
    pub fn new() -> Self {
        // §L2.10.7 row Forage: Composite{Logistic(8, 0.5), Invert} over
        // distance to nearest forageable-tile cluster. Spec line 5624:
        // 'Routine errand; sharp fall-off outside a reasonable
        // radius.' None when no forageable terrain in range — the
        // CanForage marker (eligibility) gates the DSE entirely.
        let cluster_distance = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        Self {
            id: DseId("forage"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("hunger_urgency", hangry())),
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Spatial(SpatialConsideration::new(
                    "forage_cluster_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestForageableCluster),
                    FORAGE_CLUSTER_RANGE,
                    cluster_distance,
                )),
            ],
            // RtEO weights: diligence still dominates — the point of
            // Forage vs. Hunt is diligent non-bold cats choose it.
            // Spatial axis pulls toward forageable terrain; original
            // three weights renormalized ×0.80 so spatial axis lands
            // at 0.20.
            composition: Composition::weighted_sum(vec![0.24, 0.20, 0.36, 0.20]),
            // §4 batch 2: `.require(CanForage)` gates on ¬Kitten ∧
            // ¬Injured ∧ forageable terrain nearby. Retires the
            // inline `ctx.can_forage` guard in `scoring.rs`.
            // §13.1: `.forbid(Incapacitated)` blocks downed cats.
            eligibility: EligibilityFilter::new()
                .require(markers::CanForage::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for ForageDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for ForageDse {
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
                label: "food_at_stores",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn forage_dse() -> Box<dyn Dse> {
    Box::new(ForageDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forage_dse_id_stable() {
        assert_eq!(ForageDse::new().id().0, "forage");
    }

    #[test]
    fn forage_weights_sum_to_one() {
        let dse = ForageDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn forage_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            ForageDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }
}
