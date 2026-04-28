//! Fox `Fleeing` — Fatal-threat peer (§3.3.2 anchor = 1.0). Peer of
//! cat `Flee` + `Fight` and fox `Avoiding` + `DenDefense`.
//!
//! Per §2.3 + §3.1.1: `WeightedSum` of three axes — `health_deficit`
//! via `Logistic(8, 0.5)` (injury-panic threshold), `cats_nearby` via
//! `Piecewise` step at 2+, `boldness` via `Composite { Linear(slope=
//! 0.5), Invert }` (damped invert — timid foxes flee more, but
//! boldness is a modulator, not a gate).
//!
//! Maslow tier 1 — same as fox Hunting/Raiding (survival).
//!
//! **Shape vs. inline.** Old formula:
//! `((1 - health) + cats_bonus) × (1 - bold × 0.5) × l1` with no
//! ceiling. Peak above 1.0 when `cats_nearby ≥ 2` and injured + timid.
//! Port compresses to 1.0 under RtEO so Fleeing sits at peer-group
//! magnitude.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{piecewise, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";
pub const CATS_NEARBY_INPUT: &str = "cats_nearby";
pub const BOLDNESS_INPUT: &str = "boldness";

/// §L2.10.7 fox Fleeing range — Manhattan tiles for the
/// nearest-map-edge anchor. 30 ≈ map half-extent (120/2 ≈ 60, but
/// any fox is within ~30 of an edge on a 120×90 map). Power-Invert
/// gives 'fox near edge ≈ already escaping → strong Fleeing pull.'
pub const FOX_FLEEING_EDGE_RANGE: f32 = 30.0;

pub struct FoxFleeingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxFleeingDse {
    pub fn new() -> Self {
        // Health-deficit Logistic: inflection at 0.5 matches the old
        // hardcoded `health_fraction < 0.5` gate.
        let health_curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.5,
        };
        // §2.3: step at 2+ cats. Piecewise knots (0,0),(1,0),(2,0.5),
        // (10,0.5) — cats_nearby = 0 or 1 → 0; 2+ → 0.5. This is a
        // *bonus*, not a proportional signal, so caps at 0.5.
        let cats_curve = piecewise(vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.5), (10.0, 0.5)]);
        // Damped invert: Linear(slope=0.5) maps boldness=1.0 → 0.5,
        // then Invert gives (1 - 0.5) = 0.5. Max-bold fox still
        // contributes 0.5; timid fox (bold=0) contributes 1.0.
        let boldness_curve = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 0.5,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };

        // §L2.10.7 row Fleeing: Power-Invert curve over distance to
        // nearest map edge. Spec line 5655: 'Same inverse-distance-
        // from-threat shape as cat Flee.' Anchor is map edge: closer
        // to edge = higher score = stronger Fleeing pull (the fox is
        // already escaping; the curve rewards completing the escape).
        let edge_distance = Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        };
        Self {
            id: DseId("fox_fleeing"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, health_curve)),
                Consideration::Scalar(ScalarConsideration::new(CATS_NEARBY_INPUT, cats_curve)),
                Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_curve)),
                Consideration::Spatial(SpatialConsideration::new(
                    "fox_fleeing_edge_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestMapEdge),
                    FOX_FLEEING_EDGE_RANGE,
                    edge_distance,
                )),
            ],
            // RtEO sum = 1.0. Health deficit still dominates (panic
            // when injured); cats-nearby escalates; boldness modulates.
            // Edge-distance at 0.20 mirrors the §L2.10.7 spatial-axis
            // weight precedent. Original three weights renormalized
            // by ×0.80 to make room.
            composition: Composition::weighted_sum(vec![0.36, 0.20, 0.24, 0.20]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for FoxFleeingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FoxFleeingDse {
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
                label: "fox_fled_to_safety",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn fox_fleeing_dse() -> Box<dyn Dse> {
    Box::new(FoxFleeingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_fleeing_id_stable() {
        assert_eq!(FoxFleeingDse::new().id().0, "fox_fleeing");
    }

    #[test]
    fn fox_fleeing_has_four_axes() {
        // §L2.10.7: health + cats_nearby + boldness + edge_distance.
        assert_eq!(FoxFleeingDse::new().considerations().len(), 4);
    }

    #[test]
    fn fox_fleeing_uses_nearest_map_edge_anchor() {
        let dse = FoxFleeingDse::new();
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "fox_fleeing_edge_distance" => Some(sp),
                _ => None,
            })
            .expect("fox_fleeing_edge_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::NearestMapEdge)
        ));
        // Power-Invert: closer = higher.
        assert!((spatial.curve.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!(spatial.curve.evaluate(1.0) < 1e-4);
    }

    #[test]
    fn fox_fleeing_weights_sum_to_one() {
        let sum: f32 = FoxFleeingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn fox_fleeing_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FoxFleeingDse::new().composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn fox_fleeing_maslow_tier_is_one() {
        assert_eq!(FoxFleeingDse::new().maslow_tier(), 1);
    }

    #[test]
    fn cats_nearby_steps_at_two() {
        let dse = FoxFleeingDse::new();
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Piecewise: (0,0),(1,0),(2,0.5),(10,0.5).
        assert!((c.evaluate(0.0) - 0.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.0).abs() < 1e-4);
        assert!((c.evaluate(2.0) - 0.5).abs() < 1e-4);
        assert!((c.evaluate(5.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn boldness_damped_invert() {
        let dse = FoxFleeingDse::new();
        let c = match &dse.considerations()[2] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Linear(slope=0.5) then Invert. boldness=0 → inner=0 → invert=1.
        // boldness=1 → inner=0.5 → invert=0.5.
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.5).abs() < 1e-4);
    }
}
