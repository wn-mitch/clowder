//! Fox `Hunting` — fox-side peer of cat `Hunt` in the Starvation-
//! urgency peer group (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 fox rows: `WeightedSum` of five axes —
//! `hunger_urgency`, `prey_nearby`, `prey_belief`, `day_phase`,
//! `boldness` (with `ClampMin(0.3)` floor). Maslow tier 1.
//!
//! **Shape deltas vs. the inline `score_fox_dispositions` block:**
//!
//! - Old formula: `(hunger + prey_bonus + belief + phase) ×
//!   boldness.max(0.3) × l1`. Additive-then-multiplicative; boldness
//!   is a modulator, phase_bonus is additive (can be negative at Day).
//!   Peak at starvation with prey + Night + bold = ~2.2.
//! - New formula: RtEO weighted sum of five axes, peak = 1.0 (§3.3.2
//!   peer-group anchor). Magnitude compression is intentional —
//!   §3.3.2's "peer group anchors at 1.0" contract. Cross-peer-group
//!   comparisons (Hunting vs. Fleeing) still use argmax and are
//!   magnitude-sensitive *until* Fleeing ports too (Phase 3c.2+).
//!
//! **Weight allocation rationale.** Cat `Hunt` uses `(0.5, 0.25, 0.15,
//! 0.10)` for 4 axes. Fox `Hunting` has five — boldness + day_phase
//! are the fox-specific additions. Hunger stays dominant (0.45) to
//! preserve the "starving fox picks Hunting over Fleeing" ordinal
//! while Fleeing remains on the un-ported inline formula at its old
//! magnitude. Prey_nearby held to 0.10 for the same step-function
//! reason as cat Hunt (binary 0/1 scalar — Phase 4's
//! `SpatialConsideration` on the §5.6.3 shared Prey map replaces it).
//!
//! **Day-phase shape.** §2.3: `Piecewise([(dawn, fox_hunt_dawn_bonus),
//! (day, fox_hunt_day_bonus), (dusk, fox_hunt_dusk_bonus), (night,
//! fox_hunt_night_bonus)])`. Knot *y-values* come from
//! `ScoringConstants` — passed to the constructor at registration
//! time. Runtime-mutating SimConstants does not re-parameterize the
//! DSE (same tunability limit the cat side carries today); a future
//! phase can re-register DSEs when constants change.
//!
//! **Boldness floor.** §2.3: `Composite { Linear, ClampMin(0.3) }` —
//! formalizes the old `boldness.max(0.3)`. Prevents timid foxes from
//! starving through a "refuses to hunt" lock.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{hangry, piecewise, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::resources::sim_constants::ScoringConstants;

/// Scalar-input keys (must match `fox_ctx_scalars`).
pub const HUNGER_INPUT: &str = "hunger_urgency";
pub const PREY_NEARBY_INPUT: &str = "prey_nearby";
pub const DAY_PHASE_INPUT: &str = "day_phase";
pub const BOLDNESS_INPUT: &str = "boldness";

/// §L2.10.7 fox Hunting range — Manhattan tiles for the
/// prey-belief centroid anchor. 25 ≈ a long crepuscular hunt
/// across the fox's belief grid.
pub const FOX_HUNTING_BELIEF_RANGE: f32 = 25.0;

/// Phase-to-knot encoding for the `day_phase` Piecewise curve. Must
/// match the encoding in `fox_ctx_scalars`.
pub const DAWN_KNOT: f32 = 0.0;
pub const DAY_KNOT: f32 = 0.33;
pub const DUSK_KNOT: f32 = 0.66;
pub const NIGHT_KNOT: f32 = 1.0;

pub struct FoxHuntingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxHuntingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        let day_phase_curve = piecewise(vec![
            (DAWN_KNOT, scoring.fox_hunt_dawn_bonus),
            (DAY_KNOT, scoring.fox_hunt_day_bonus),
            (DUSK_KNOT, scoring.fox_hunt_dusk_bonus),
            (NIGHT_KNOT, scoring.fox_hunt_night_bonus),
        ]);
        let boldness_floor = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            }),
            post: PostOp::ClampMin(0.3),
        };
        // §L2.10.7 row Hunting: Quadratic over distance to the
        // prey-belief cluster centroid. Spec line 5648:
        // 'Belief-grid provides soft location; same ch14 shape as
        // cat Hunt.' `Composite { Quadratic(exp=2, divisor=1, shift=0),
        // Invert }` evaluates `1 - cost^2`: at the centroid → 1,
        // half-distance → 0.75, range edge → 0. Replaces the retired
        // `prey_belief` Linear scalar axis.
        let belief_distance = Curve::Composite {
            inner: Box::new(Curve::Quadratic {
                exponent: 2.0,
                divisor: 1.0,
                shift: 0.0,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("fox_hunting"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_INPUT, hangry())),
                Consideration::Scalar(ScalarConsideration::new(
                    PREY_NEARBY_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Spatial(SpatialConsideration::new(
                    "fox_hunting_belief_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::PreyBeliefCentroid),
                    FOX_HUNTING_BELIEF_RANGE,
                    belief_distance,
                )),
                Consideration::Scalar(ScalarConsideration::new(DAY_PHASE_INPUT, day_phase_curve)),
                Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_floor)),
            ],
            // RtEO sum = 1.0. Same layout as the pre-port shape with
            // the prey_belief scalar replaced by the spatial axis;
            // weights unchanged because the new axis carries the same
            // 'where is the prey' signal as the retired scalar.
            composition: Composition::weighted_sum(vec![0.45, 0.10, 0.10, 0.10, 0.25]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for FoxHuntingDse {
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

pub fn fox_hunting_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(FoxHuntingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_hunting_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(FoxHuntingDse::new(&s).id().0, "fox_hunting");
    }

    #[test]
    fn fox_hunting_has_five_axes() {
        let s = ScoringConstants::default();
        assert_eq!(FoxHuntingDse::new(&s).considerations().len(), 5);
    }

    #[test]
    fn fox_hunting_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = FoxHuntingDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
    }

    #[test]
    fn fox_hunting_is_weighted_sum() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            FoxHuntingDse::new(&s).composition().mode,
            CompositionMode::WeightedSum
        );
    }

    #[test]
    fn fox_hunting_is_maslow_tier_1() {
        let s = ScoringConstants::default();
        assert_eq!(FoxHuntingDse::new(&s).maslow_tier(), 1);
    }

    fn scalar_curve<'a>(dse: &'a FoxHuntingDse, name: &str) -> &'a Curve {
        dse.considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(sc) if sc.name == name => Some(&sc.curve),
                _ => None,
            })
            .unwrap_or_else(|| panic!("scalar axis {name} must exist"))
    }

    #[test]
    fn boldness_floor_prevents_zero_contribution() {
        // ClampMin(0.3): even a fox with boldness=0.0 contributes 0.3
        // through the Linear identity. That's the §2.3 "timid fox still
        // hunts when starving" anchor.
        let s = ScoringConstants::default();
        let dse = FoxHuntingDse::new(&s);
        let boldness_curve = scalar_curve(&dse, BOLDNESS_INPUT);
        assert!((boldness_curve.evaluate(0.0) - 0.3).abs() < 1e-4);
        assert!((boldness_curve.evaluate(0.5) - 0.5).abs() < 1e-4);
        assert!((boldness_curve.evaluate(1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn day_phase_curve_evaluates_knot_bonuses() {
        // At each phase-encoded knot the Piecewise should return the
        // exact ScoringConstants bonus. Verifies the knot encoding
        // matches `fox_ctx_scalars`.
        let s = ScoringConstants::default();
        let dse = FoxHuntingDse::new(&s);
        let curve = scalar_curve(&dse, DAY_PHASE_INPUT);
        assert!((curve.evaluate(DAWN_KNOT) - s.fox_hunt_dawn_bonus).abs() < 1e-4);
        assert!((curve.evaluate(DAY_KNOT) - s.fox_hunt_day_bonus).abs() < 1e-4);
        assert!((curve.evaluate(DUSK_KNOT) - s.fox_hunt_dusk_bonus).abs() < 1e-4);
        assert!((curve.evaluate(NIGHT_KNOT) - s.fox_hunt_night_bonus).abs() < 1e-4);
    }

    #[test]
    fn fox_hunting_uses_prey_belief_centroid_anchor() {
        let s = ScoringConstants::default();
        let dse = FoxHuntingDse::new(&s);
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "fox_hunting_belief_distance" => Some(sp),
                _ => None,
            })
            .expect("fox_hunting_belief_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::PreyBeliefCentroid)
        ));
        // Quadratic-Invert: closer = higher.
        assert!((spatial.curve.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((spatial.curve.evaluate(0.5) - 0.75).abs() < 1e-4);
        assert!(spatial.curve.evaluate(1.0) < 1e-4);
    }

    #[test]
    fn fox_hunting_dse_boxed_registers() {
        let s = ScoringConstants::default();
        assert_eq!(fox_hunting_dse(&s).id().0, "fox_hunting");
    }
}
