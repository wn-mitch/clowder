//! `Build` — Work-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1493 (post-§L2.10.7): `WeightedSum` of 3
//! axes — diligence (Linear), site_distance
//! (`Composite{Logistic(8, 0.5), Invert}` over distance to nearest
//! construction site, replacing the retired binary `has_construction_site`
//! Piecewise axis), repair_presence (Piecewise `(0, 0),
//! (1, build_repair_bonus)`).
//! RtEO: site proximity drives even low-diligence cats ("there's
//! literally a half-built wall here"); repair need drives build
//! independently.
//!
//! Maslow tier 2 — Build is a safety-infrastructure action that
//! shouldn't be gated on pre-existing safety (chicken-and-egg per
//! the old inline comment), but a hungry cat still shouldn't build.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{piecewise, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub const DILIGENCE_INPUT: &str = "diligence";
pub const SITE_PRESENCE_INPUT: &str = "has_construction_site";
pub const REPAIR_PRESENCE_INPUT: &str = "has_damaged_building";

/// §L2.10.7 Build range — Manhattan tiles for the
/// nearest-construction-site anchor. 25 ≈ a long colony walk;
/// matches Cook/Eat/Farm range cluster.
pub const BUILD_SITE_RANGE: f32 = 25.0;

pub struct BuildDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl BuildDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // §L2.10.7 row Build: Composite{Logistic(8, 0.5), Invert} over
        // distance to the nearest construction site. Replaces the
        // binary `has_construction_site` Piecewise axis — distance to
        // the work IS the presence signal (None when no site nearby
        // → CP/WS gate suppresses the build score). The `build_site_bonus`
        // tunable retires; the curve's plateau gives the same "literally
        // a half-built wall here" pull at close range.
        let site_distance = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        Self {
            id: DseId("build"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    DILIGENCE_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Spatial(SpatialConsideration::new(
                    "build_site_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestConstructionSite),
                    BUILD_SITE_RANGE,
                    site_distance,
                )),
                // `has_damaged_building` retains its binary Piecewise
                // shape today: §L2.10.7's roster commits one landmark
                // per row (Site position), and damaged-building repair
                // is a distinct repair-pull signal that isn't named
                // separately in the spec. Future audit may split this
                // into a NearestDamagedBuilding anchor.
                Consideration::Scalar(ScalarConsideration::new(
                    REPAIR_PRESENCE_INPUT,
                    piecewise(vec![(0.0, 0.0), (1.0, scoring.build_repair_bonus)]),
                )),
            ],
            // RtEO sum = 1.0. Diligence is primary; spatial axis pulls
            // toward the site; repair-presence is the auxiliary repair
            // bonus.
            composition: Composition::weighted_sum(vec![0.5, 0.25, 0.25]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Dse for BuildDse {
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
                label: "built_or_repaired",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn build_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(BuildDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(BuildDse::new(&s).id().0, "build");
    }

    #[test]
    fn build_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = BuildDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}
