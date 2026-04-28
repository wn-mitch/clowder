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
            ],
            // RtEO sum = 1.0. Directive count drives, diligence +
            // ambition modulate, perch proximity pulls toward the
            // coordination location. Original three weights renormalized
            // ×0.80 to make room for the spatial axis at 0.20.
            composition: Composition::weighted_sum(vec![0.24, 0.32, 0.24, 0.20]),
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

pub fn coordinate_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
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
}
