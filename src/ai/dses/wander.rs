//! `Wander` — Exploration-urgency peer (§3.3.2 anchor = 1.0). The
//! spec's canonical WS example (§3.1 summary), because its base_rate
//! axis exemplifies the "keep available at zero drive" RtEO pattern.
//!
//! Per §2.3 + §3.1.1 row 1502: `WeightedSum` of 3 axes — curiosity
//! (Linear), base_rate (Linear with intercept = wander_base),
//! playfulness (Linear, additive bonus). §3.3.2 row note: "Wander
//! caps below Explore" (Wander is a base-rate fallback when nothing
//! unexplored is nearby).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, FieldConsideration, FieldSource, LandmarkAnchor, LandmarkSource,
    ScalarConsideration,
};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    ActivityKind, CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention,
    Termination,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub const CURIOSITY_INPUT: &str = "curiosity";
pub const ONE_INPUT: &str = "one";
pub const PLAYFULNESS_INPUT: &str = "playfulness";

/// Manhattan range over which Wander's route-cost axis normalizes
/// the cost-to-reach the candidate wander tile (pre-picked at score
/// time via `LandmarkAnchor::WanderTargetAnchor`). 20 tiles ≈ a
/// short stroll; matches the seeded-offset radius cap (8 + 12 ×
/// curiosity).
pub const WANDER_TARGET_RANGE: f32 = 20.0;

pub struct WanderDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl WanderDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let base_curve = Curve::Linear {
            slope: 0.0,
            intercept: scoring.wander_base,
        };
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // 228: destination-aware route-cost axis. Reads OwnRouteCost
        // at WanderTargetAnchor — a deterministic seeded offset
        // pre-picked at score time so Wander has a destination to
        // price route cost against. Curve `Composite{Logistic(8.0,
        // 0.5), Invert}` mirrors Forage's shape — Wander is a
        // routine errand and the falloff should be sharp past the
        // half-range. Ships dormant at 0.0; tuning is a follow-on.
        let route_cost_weight = scoring.wander_route_cost_weight.clamp(0.0, 1.0);
        let route_cost_curve = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        let route_cost_remainder = 1.0 - route_cost_weight;
        Self {
            id: DseId("wander"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(CURIOSITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(ONE_INPUT, base_curve)),
                Consideration::Scalar(ScalarConsideration::new(PLAYFULNESS_INPUT, linear)),
                Consideration::Field(FieldConsideration::new(
                    "wander_route_cost",
                    FieldSource::OwnRouteCost,
                    LandmarkSource::Anchor(LandmarkAnchor::WanderTargetAnchor),
                    WANDER_TARGET_RANGE,
                    route_cost_curve,
                )),
            ],
            // RtEO sum = 1.0. Curiosity dominates; base_rate keeps
            // Wander available at zero curiosity; playfulness rider.
            // Route-cost scales the others by its remainder.
            composition: Composition::weighted_sum(vec![
                0.5 * route_cost_remainder,
                0.2 * route_cost_remainder,
                0.3 * route_cost_remainder,
                route_cost_weight,
            ]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Dse for WanderDse {
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
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Wander,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}
impl crate::ai::dse::CatDse for WanderDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Wander
    }

    fn always_emit_zero(&self) -> bool {
        true
    }
}

pub fn wander_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(WanderDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wander_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(WanderDse::new(&s).id().0, "wander");
    }

    #[test]
    fn wander_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = WanderDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn wander_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            WanderDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn wander_route_cost_dormant_at_default_zero() {
        // 228: at default `wander_route_cost_weight = 0.0`, the
        // route-cost axis contributes zero. Consideration always
        // present (WS mode tolerates weight-zero axes).
        let s = ScoringConstants::default();
        assert!((s.wander_route_cost_weight).abs() < 1e-6);
        let dse = WanderDse::new(&s);
        let weights = &dse.composition().weights;
        assert_eq!(weights.len(), 4);
        assert!((weights[3]).abs() < 1e-6);
        let has_axis = dse.considerations().iter().any(|c| match c {
            Consideration::Field(f) => f.name == "wander_route_cost",
            _ => false,
        });
        assert!(has_axis);
    }

    #[test]
    fn wander_route_cost_scales_others_when_weight_nonzero() {
        let mut s = ScoringConstants::default();
        s.wander_route_cost_weight = 0.4;
        let dse = WanderDse::new(&s);
        let weights = &dse.composition().weights;
        // Original 0.5 weight scales to 0.5 × 0.6 = 0.3.
        assert!((weights[0] - 0.3).abs() < 1e-4);
        assert!((weights[3] - 0.4).abs() < 1e-4);
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static WANDER_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 1000,
        construct: |s| wander_dse(s),
    };
