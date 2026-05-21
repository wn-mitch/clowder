//! `Hunt` — peer of Eat/Forage in the Starvation-urgency group.
//!
//! Per §2.3 + §3.1.1: WeightedSum of 4 axes — `hunger_urgency`,
//! `food_scarcity`, `boldness`, `prey_nearby`.
//!
//! Design intent per §3.1.1: "Bold cat spotting prey ⇒ Hunt even on
//! full stomach; starving timid cat ⇒ Hunt out of need. No single
//! axis is a gate."
//!
//! **Prey-proximity shape.** §2.3 specifies the end-state as a
//! `SpatialConsideration` sampling the Prey-location map with
//! `Quadratic(exponent=2)` falloff. Today's code uses
//! `bool prey_nearby + additive hunt_prey_bonus` — a binary
//! presence signal. For Phase 3c.1a we keep the binary signal as
//! a scalar (0 or 1) fed through `Linear(slope=1, intercept=0)`,
//! preserving the old behavior. The spatial upgrade moves to Phase
//! 4 alongside `TargetTakingDse` / `SpatialConsideration` wiring
//! per §6.3 / §L2.10.7.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, FieldConsideration, FieldSource, LandmarkAnchor, LandmarkSource,
    ScalarConsideration,
};
use crate::ai::curves::{hangry, scarcity, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

/// Manhattan range over which Hunt's route-cost axis normalizes
/// the cost-to-reach the nearest prey scent peak. 30 tiles ≈ a
/// medium hunt circuit. Tiles farther (or unreachable) score the
/// axis at zero and Hunt is suppressed proportionally.
pub const HUNT_PREY_RANGE: f32 = 30.0;

pub struct HuntDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HuntDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // 176: colony-food-security saturation axis. The composite
        // curve (Logistic 8.0/0.5 → Invert) produces a high score
        // when colony food security is LOW and a low score when it's
        // HIGH — i.e., as the colony moves up the Maslow ladder, the
        // saturation axis stops contributing to the hunt sum, freeing
        // L3 bandwidth for higher-tier DSEs. Default weight 0.0 keeps
        // the axis dormant; balance-tuning lifts the weight in a
        // follow-on once the saturation behavior is observed.
        let saturation_curve = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        // RtEO weight redistribution: when balance-tuning sets a
        // non-zero `hunt_food_security_weight`, the existing four
        // weights still sum to 1.0 minus the saturation weight (the
        // saturation axis is the new fifth), preserving the RtEO
        // invariant. Stage 5 ships with weight 0.0 so the existing
        // four weights stay at their canonical 0.5/0.25/0.15/0.10.
        let saturation_weight = scoring.hunt_food_security_weight.clamp(0.0, 1.0);
        // 228: destination-aware route-cost axis. Reads OwnRouteCost
        // at NearestPreyAnchor — the cat's flooded path-cost to the
        // nearest prey-scent peak (terrain + boldness-weighted
        // fox-scent + corruption). Curve `Composite{Quadratic(2),
        // Invert}` — sharper falloff than Forage's Logistic shape
        // because Hunt is the urgency-tier action and a high-cost
        // route should suppress it more aggressively than a routine
        // errand. Closer-is-better via Invert. Ships dormant at 0.0;
        // tuning is a follow-on ticket.
        let route_cost_weight = scoring.hunt_route_cost_weight.clamp(0.0, 1.0);
        let route_cost_curve = Curve::Composite {
            inner: Box::new(Curve::Quadratic {
                exponent: 2.0,
                divisor: 1.0,
                shift: 0.0,
            }),
            post: PostOp::Invert,
        };
        let route_cost_remainder = 1.0 - route_cost_weight;
        let remainder = (1.0 - saturation_weight) * route_cost_remainder;
        let saturation_term = saturation_weight * route_cost_remainder;
        Self {
            id: DseId("hunt"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("hunger_urgency", hangry())),
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "boldness",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    "prey_nearby",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    "colony_food_security",
                    saturation_curve,
                )),
                Consideration::Field(FieldConsideration::new(
                    "hunt_route_cost",
                    FieldSource::OwnRouteCost,
                    LandmarkSource::Anchor(LandmarkAnchor::NearestPreyAnchor),
                    HUNT_PREY_RANGE,
                    route_cost_curve,
                )),
            ],
            // RtEO weights sum to 1.0. Hunger dominates so starving
            // cats converge on food-acquisition DSEs (peer-group
            // anchor §3.3.2). Scarcity and boldness follow;
            // prey_nearby held to 0.10. The saturation and
            // route-cost axes scale the others by their respective
            // remainders so the sum stays 1.0 at all weight values.
            composition: Composition::weighted_sum(vec![
                0.5 * remainder,
                0.25 * remainder,
                0.15 * remainder,
                0.10 * remainder,
                saturation_term,
                route_cost_weight,
            ]),
            // §4 batch 2: `.require(CanHunt)` gates on (Adult ∨ Young)
            // ∧ ¬InCombat ∧ forest nearby. Ticket 184 (2026-05-06)
            // dropped the `¬Injured` clause: injured cats can still
            // elect Hunt — the skill/health scoring signals already
            // dampen the L2 appeal, so the eligibility gate was
            // double-counting and the absence of Hunt for injured
            // cats shifted action-share to Patrol's Blind-commitment
            // long plans.
            // §13.1: `.forbid(Incapacitated)` blocks downed cats.
            // §9.3 stance binding migrated to `hunt_target_dse` —
            // candidate-prefilter happens in the target-taking resolver.
            eligibility: EligibilityFilter::new()
                .require(markers::CanHunt::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Dse for HuntDse {
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
                label: "prey_caught",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}
impl crate::ai::dse::CatDse for HuntDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Hunt
    }
}


pub fn hunt_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(HuntDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunt_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(HuntDse::new(&s).id().0, "hunt");
    }

    #[test]
    fn hunt_has_six_axes() {
        // 176: colony_food_security saturation axis appended.
        // 228: hunt_route_cost Field axis appended.
        let s = ScoringConstants::default();
        assert_eq!(HuntDse::new(&s).considerations().len(), 6);
    }

    #[test]
    fn hunt_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let dse = HuntDse::new(&s);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
    }

    #[test]
    fn hunt_saturation_dormant_at_default_zero() {
        // 176: with default `hunt_food_security_weight = 0.0`, the
        // saturation axis contributes zero to the weighted sum
        // regardless of input. The other four axes retain their
        // canonical RtEO weights (0.5 / 0.25 / 0.15 / 0.10).
        let s = ScoringConstants::default();
        assert!((s.hunt_food_security_weight).abs() < 1e-6);
        let dse = HuntDse::new(&s);
        let weights = &dse.composition().weights;
        assert!((weights[0] - 0.5).abs() < 1e-4);
        assert!((weights[4]).abs() < 1e-6);
    }

    #[test]
    fn hunt_route_cost_dormant_at_default_zero() {
        // 228: with default `hunt_route_cost_weight = 0.0`, the
        // route-cost axis contributes zero. Consideration always
        // present (WS mode tolerates weight-zero axes).
        let s = ScoringConstants::default();
        assert!((s.hunt_route_cost_weight).abs() < 1e-6);
        let dse = HuntDse::new(&s);
        let weights = &dse.composition().weights;
        assert!((weights[5]).abs() < 1e-6);
        let has_axis = dse.considerations().iter().any(|c| match c {
            Consideration::Field(f) => f.name == "hunt_route_cost",
            _ => false,
        });
        assert!(has_axis);
    }

    #[test]
    fn hunt_route_cost_scales_others_when_weight_nonzero() {
        let mut s = ScoringConstants::default();
        s.hunt_route_cost_weight = 0.2;
        let dse = HuntDse::new(&s);
        let weights = &dse.composition().weights;
        // Original 0.5 weight scales to 0.5 × 0.8 = 0.4.
        assert!((weights[0] - 0.4).abs() < 1e-4);
        assert!((weights[5] - 0.2).abs() < 1e-4);
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static HUNT_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 300,
        construct: |s| hunt_dse(s),
    };
