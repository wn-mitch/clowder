//! Fox `Dispersing` — Lifecycle-override peer (§3.3.2 anchor = 2.0,
//! single-member group). Intentionally exceeds every other fox
//! disposition's 1.0 peak so Dispersing cannot be outvoted when its
//! eligibility filter (juvenile-homeless-fox) fires.
//!
//! Per §2.3 row 1140 + §3.1.1 row 1534: `CompensatedProduct` of 1
//! axis — a `Linear(intercept=2.0)` lifecycle intercept. n=1 CP is
//! degenerate but locks the peer-group contract. The juvenile-
//! dispersal lifecycle marker serves as the §4 eligibility filter
//! when markers land in Phase 3d — for now the outer
//! `is_dispersing_juvenile` gate in `score_fox_dispositions` handles
//! eligibility.
//!
//! Maslow opt-out (`u8::MAX`) — dispersal is a lifecycle-stage
//! instinct, unsuppressed by normal Maslow layering.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const ONE_INPUT: &str = "one";

/// §L2.10.7 fox Dispersing range — Manhattan tiles for the
/// unexplored-frontier centroid anchor. 40 ≈ map half-extent;
/// disperser foxes drift outward across the colony scale.
pub const FOX_DISPERSING_FRONTIER_RANGE: f32 = 40.0;

pub struct FoxDispersingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxDispersingDse {
    pub fn new() -> Self {
        // §L2.10.7 row Dispersing: Linear over normalized distance to
        // unexplored frontier centroid. Spec line 5654: 'Gradient-
        // following from parent territory outward.' Linear(slope=-1,
        // intercept=1) → closer-to-frontier scores higher; the
        // gradient draws the juvenile fox toward unclaimed territory.
        let frontier_distance = Curve::Linear {
            slope: -1.0,
            intercept: 1.0,
        };
        Self {
            id: DseId("fox_dispersing"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    ONE_INPUT,
                    // Linear(slope=0, intercept=2.0) — the lifecycle
                    // anchor value. Composite for spec fidelity (the §2.3
                    // row's `Linear(intercept=2.0)` + Composite's lifted
                    // envelope per `curves.rs` module doc).
                    Curve::Composite {
                        inner: Box::new(Curve::Linear {
                            slope: 0.0,
                            intercept: 2.0,
                        }),
                        post: crate::ai::curves::PostOp::ClampMin(2.0),
                    },
                )),
                Consideration::Spatial(SpatialConsideration::new(
                    "fox_dispersing_frontier_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::UnexploredFrontierCentroid),
                    FOX_DISPERSING_FRONTIER_RANGE,
                    frontier_distance,
                )),
            ],
            // CP 2-axis with both weights at 1.0. Lifecycle axis stays
            // at peak 2.0 (per §3.3.2 Dispersing anchor); spatial axis
            // multiplies to 1.0 at the frontier centroid (preserving
            // the override). Far from the frontier the spatial axis
            // attenuates and Dispersing's effective score drops, but
            // the eligibility filter (juvenile-homeless) gates the DSE
            // entirely so non-juveniles never see it.
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for FoxDispersingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FoxDispersingDse {
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
        CommitmentStrategy::Blind
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "dispersed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::Blind,
        }
    }
    fn maslow_tier(&self) -> u8 {
        u8::MAX
    }
}

pub fn fox_dispersing_dse() -> Box<dyn Dse> {
    Box::new(FoxDispersingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_dispersing_id_stable() {
        assert_eq!(FoxDispersingDse::new().id().0, "fox_dispersing");
    }

    #[test]
    fn fox_dispersing_opts_out_of_maslow() {
        assert_eq!(FoxDispersingDse::new().maslow_tier(), u8::MAX);
    }

    #[test]
    fn fox_dispersing_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FoxDispersingDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn fox_dispersing_curve_evaluates_to_two() {
        let dse = FoxDispersingDse::new();
        // Find the lifecycle scalar by name — §L2.10.7 added a 2nd
        // (Spatial) consideration so the index would shift.
        let c = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(sc) if sc.name == ONE_INPUT => Some(&sc.curve),
                _ => None,
            })
            .expect("lifecycle axis must exist");
        // The Composite{Linear(intercept=2.0), ClampMin(2.0)}
        // envelope outputs 2.0 regardless of input.
        assert!((c.evaluate(0.0) - 2.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn fox_dispersing_uses_frontier_centroid_anchor() {
        let dse = FoxDispersingDse::new();
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "fox_dispersing_frontier_distance" => {
                    Some(sp)
                }
                _ => None,
            })
            .expect("fox_dispersing_frontier_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::UnexploredFrontierCentroid)
        ));
    }
}
