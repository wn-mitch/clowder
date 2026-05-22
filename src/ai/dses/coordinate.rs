//! `Coordinate` — Work-urgency peer (§3.3.2 anchor = 1.0). Scored
//! only for cats with active coordinator directives.
//!
//! Per §2.3 + §3.1.1 row 1506: `WeightedSum` of 3 axes — diligence
//! (Linear), pending_directive_count via `Composite { Linear(slope=
//! coordinate_directive_scale), Clamp(max=cap) }` (saturating-count
//! anchor — one vs. ten directives shouldn't produce a 10× score),
//! ambition (Linear).
//!
//! Eligibility: `is_coordinator_with_directives` (outer gate).
//! Maslow tier 4 per the old inline (self-esteem tier —
//! coordination is respect-seeking work).

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
use crate::resources::sim_constants::ScoringConstants;

pub const DILIGENCE_INPUT: &str = "diligence";
pub const DIRECTIVE_COUNT_INPUT: &str = "pending_directive_count";
pub const AMBITION_INPUT: &str = "ambition";

/// §L2.10.7 Coordinate range — Manhattan tiles for the coordinator-
/// perch anchor. 18 ≈ inner-colony walking distance.
pub const COORDINATE_PERCH_RANGE: f32 = 18.0;

pub struct CoordinateDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CoordinateDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let directive_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: scoring.coordinate_directive_scale,
                intercept: 0.0,
            }),
            post: PostOp::ClampMax(1.0),
        };
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // §L2.10.7 row Coordinate: Composite{Logistic(8, 0.5), Invert}
        // over distance to the coordinator's perch. Spec line 5637:
        // 'Weakly spatial — coordinator works from location; distant
        // cats discounted for participation.' Logistic for routine-
        // commute plateau.
        let perch_distance = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        // 209: positive `colony_food_security` axis. Plain Logistic
        // (no Invert) — output rises with food security. Default
        // weight 0.0 ships dormant.
        let lift_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        let lift_weight = scoring.coordinate_food_security_weight.clamp(0.0, 1.0);
        let remainder = 1.0 - lift_weight;
        Self {
            id: DseId("coordinate"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(DILIGENCE_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(
                    DIRECTIVE_COUNT_INPUT,
                    directive_curve,
                )),
                Consideration::Scalar(ScalarConsideration::new(AMBITION_INPUT, linear)),
                Consideration::Spatial(SpatialConsideration::new(
                    "coordinate_perch_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::CoordinatorPerch),
                    COORDINATE_PERCH_RANGE,
                    perch_distance,
                )),
                Consideration::Scalar(ScalarConsideration::new("colony_food_security", lift_curve)),
            ],
            // RtEO sum = 1.0. Directive count drives, diligence +
            // ambition modulate, perch proximity pulls toward the
            // coordination location. The fifth axis
            // (colony_food_security) ships at default-zero weight; the
            // other four scale by `remainder` so the weight sum stays
            // 1.0 even when balance-tuning lifts the lift knob.
            composition: Composition::weighted_sum(vec![
                0.24 * remainder,
                0.32 * remainder,
                0.24 * remainder,
                0.20 * remainder,
                lift_weight,
            ]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            // §4: only coordinators with pending directives are eligible.
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .require(markers::IsCoordinatorWithDirectives::KEY),
        }
    }
}

impl Dse for CoordinateDse {
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
                label: "directives_delivered",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        4
    }
}
impl crate::ai::dse::CatDse for CoordinateDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Coordinate
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn coordinate_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(CoordinateDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(CoordinateDse::new(&s).id().0, "coordinate");
    }

    #[test]
    fn coordinate_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = CoordinateDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn coordinate_food_security_tuned_to_iter1_weight() {
        // 211 iter-1: weight 0.10. The (1-w) rebalance scales the
        // existing four weights to 0.216/0.288/0.216/0.180 and the new
        // fifth axis carries 0.10, summing to 1.0.
        let s = ScoringConstants::default();
        assert!((s.coordinate_food_security_weight - 0.10).abs() < 1e-4);
        let weights = CoordinateDse::new(&s).composition().weights.clone();
        assert_eq!(weights.len(), 5);
        assert!((weights[0] - 0.216).abs() < 1e-4);
        assert!((weights[1] - 0.288).abs() < 1e-4);
        assert!((weights[2] - 0.216).abs() < 1e-4);
        assert!((weights[3] - 0.180).abs() < 1e-4);
        assert!((weights[4] - 0.10).abs() < 1e-4);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static COORDINATE_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 2600,
        construct: |s| coordinate_dse(s),
    };
