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
    Consideration, FieldConsideration, FieldSource, LandmarkAnchor, LandmarkSource,
    ScalarConsideration, SpatialConsideration,
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
        // 228: destination-aware route-cost axis. Reads OwnRouteCost
        // at NearestForageableCluster — the cat's flooded path cost
        // to the forageable patch (terrain + boldness-weighted
        // fox-scent + corruption). Curve `Composite{Logistic(8, 0.5),
        // Invert}` mirrors the Spatial cluster_distance shape so
        // dormant runs stay close to baseline; tuning the weight
        // up makes Forage suppress when the route to forage is
        // costly. Ships dormant at 0.0; tuning is a follow-on.
        let route_cost_weight = scoring.forage_route_cost_weight.clamp(0.0, 1.0);
        let route_cost_curve = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        let route_cost_remainder = 1.0 - route_cost_weight;
        let remainder = (1.0 - saturation_weight) * route_cost_remainder;
        let saturation_term = saturation_weight * route_cost_remainder;
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
                Consideration::Field(FieldConsideration::new(
                    "forage_route_cost",
                    FieldSource::OwnRouteCost,
                    LandmarkSource::Anchor(LandmarkAnchor::NearestForageableCluster),
                    FORAGE_CLUSTER_RANGE,
                    route_cost_curve,
                )),
            ],
            // RtEO weights: diligence still dominates — the point of
            // Forage vs. Hunt is diligent non-bold cats choose it.
            // Spatial axis pulls toward forageable terrain; the
            // saturation and route-cost axes scale the others by
            // their remainders. Ships dormant (both extra weights at
            // 0) so the original four weights stay canonical.
            composition: Composition::weighted_sum(vec![
                0.24 * remainder,
                0.20 * remainder,
                0.36 * remainder,
                0.20 * remainder,
                saturation_term,
                route_cost_weight,
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
        // other axes retain their canonical RtEO weights.
        let s = ScoringConstants::default();
        assert!((s.forage_food_security_weight).abs() < 1e-6);
        let dse = ForageDse::new(&s);
        let weights = &dse.composition().weights;
        assert!((weights[0] - 0.24).abs() < 1e-4);
        assert!((weights[4]).abs() < 1e-6);
        // 228: route-cost axis added; six axes total at default.
        assert_eq!(dse.considerations().len(), 6);
    }

    #[test]
    fn forage_route_cost_dormant_at_default_zero() {
        // 228: with default `forage_route_cost_weight = 0.0`, the
        // route-cost axis contributes zero to the weighted sum but
        // the consideration is always present (WS mode tolerates
        // weight-zero axes; consistent shape avoids per-DSE
        // conditional add).
        let s = ScoringConstants::default();
        assert!((s.forage_route_cost_weight).abs() < 1e-6);
        let dse = ForageDse::new(&s);
        let weights = &dse.composition().weights;
        assert!((weights[5]).abs() < 1e-6);
        let has_axis = dse.considerations().iter().any(|c| match c {
            Consideration::Field(f) => f.name == "forage_route_cost",
            _ => false,
        });
        assert!(has_axis);
    }

    #[test]
    fn forage_route_cost_axis_scales_others_when_weight_nonzero() {
        // Symmetric: when balance-tuning lifts the route-cost weight,
        // the existing weights scale by (1 - route_cost_weight) to
        // preserve RtEO sum=1.0.
        let mut s = ScoringConstants::default();
        s.forage_route_cost_weight = 0.25;
        let dse = ForageDse::new(&s);
        let weights = &dse.composition().weights;
        // Original 0.24 weight scales to 0.24 × 0.75 = 0.18.
        assert!((weights[0] - 0.18).abs() < 1e-4);
        assert!((weights[5] - 0.25).abs() < 1e-4);
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}
