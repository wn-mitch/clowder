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
use crate::resources::sim_constants::ScoringConstants;

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
    pub fn new(scoring: &ScoringConstants) -> Self {
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
        // 176: colony-food-security saturation axis (sibling to
        // Hunt's). Composite (Logistic 8.0/0.5 → Invert): high score
        // when colony food security is LOW, low score when it's HIGH
        // — so as Maslow tier 1 saturates, Forage stops contributing
        // and L3 bandwidth flows to higher-tier DSEs.
        let saturation_curve = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        let saturation_weight = scoring.forage_food_security_weight.clamp(0.0, 1.0);
        let remainder = 1.0 - saturation_weight;
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
                Consideration::Scalar(ScalarConsideration::new(
                    "colony_food_security",
                    saturation_curve,
                )),
            ],
            // RtEO weights: diligence still dominates — the point of
            // Forage vs. Hunt is diligent non-bold cats choose it.
            // Spatial axis pulls toward forageable terrain; original
            // four weights renormalized ×remainder so the new fifth
            // (colony_food_security) lands at `saturation_weight`.
            // Stage 5 ships saturation_weight = 0.0 (default-zero), so
            // the original weights stay at their canonical values.
            composition: Composition::weighted_sum(vec![
                0.24 * remainder,
                0.20 * remainder,
                0.36 * remainder,
                0.20 * remainder,
                saturation_weight,
            ]),
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

pub fn forage_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(ForageDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forage_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(ForageDse::new(&s).id().0, "forage");
    }

    #[test]
    fn forage_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let dse = ForageDse::new(&s);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn forage_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            ForageDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn forage_saturation_dormant_at_default_zero() {
        // 176: with default `forage_food_security_weight = 0.0`, the
        // saturation axis contributes zero to the weighted sum. The
        // other four axes retain their canonical RtEO weights.
        let s = ScoringConstants::default();
        assert!((s.forage_food_security_weight).abs() < 1e-6);
        let dse = ForageDse::new(&s);
        let weights = &dse.composition().weights;
        assert!((weights[0] - 0.24).abs() < 1e-4);
        assert!((weights[4]).abs() < 1e-6);
        assert_eq!(dse.considerations().len(), 5);
    }
}
