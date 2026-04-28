//! `Patrol` (cat) — Fatal-threat peer AND Territory-urgency peer
//! (§3.3.2 dual-listed). Proactive safety-seeking — the
//! above-threshold cousin of `Flee`.
//!
//! Per §2.3 + §3.1.1 row 1492: `CompensatedProduct` of 3 axes —
//! `safety_deficit` via `Logistic(6, patrol_safety_threshold)`
//! (softer than Flee's steepness=10 — Patrol is proactive, operates
//! above Flee's threshold), `boldness` via Linear, and
//! `safety_upper_bound` via `Composite{Logistic(20, patrol_exit_threshold), Invert}`
//! — an upper gate that zeros Patrol's score when safety has
//! recovered past the exit threshold. Three gates: timid cats flee
//! instead of patrol; full-safety has nothing to patrol; safety-sated
//! cats stop picking Patrol at re-evaluation. The third axis closes
//! the Thistle-pattern Patrol loop (seed-18301685438630318625 soak) —
//! without it, Patrol kept winning in the 0.35–0.8 safety band even
//! after the §7.2 commitment gate dropped the held Guarding plan.
//! Maslow tier 2.

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

pub const SAFETY_DEFICIT_INPUT: &str = "safety_deficit";
pub const BOLDNESS_INPUT: &str = "boldness";
pub const SAFETY_UPPER_BOUND_INPUT: &str = "safety";

/// §L2.10.7 Patrol range — Manhattan tiles for the territory
/// perimeter anchor. 25 ≈ same scale as HerbcraftWard's perimeter
/// range (both target the colony perimeter).
pub const PATROL_PERIMETER_RANGE: f32 = 25.0;

pub struct PatrolDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PatrolDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        Self {
            id: DseId("patrol"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    SAFETY_DEFICIT_INPUT,
                    Curve::Logistic {
                        steepness: 6.0,
                        midpoint: scoring.patrol_safety_threshold,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    BOLDNESS_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                // Upper-bound gate: reads `safety` (not deficit) with
                // a sharp Logistic inverted — outputs ~1 when safety
                // is below `patrol_exit_threshold` and ~0 above.
                // Multiplied into the CompensatedProduct, this zeros
                // Patrol's score when safety has recovered. See
                // `docs/balance/guarding-exit-recipe.md` iter 2.
                Consideration::Scalar(ScalarConsideration::new(
                    SAFETY_UPPER_BOUND_INPUT,
                    Curve::Composite {
                        inner: Box::new(Curve::Logistic {
                            steepness: 20.0,
                            midpoint: scoring.patrol_exit_threshold,
                        }),
                        post: PostOp::Invert,
                    },
                )),
                // §L2.10.7 row Patrol: Linear over normalized distance
                // to the territory perimeter anchor. Spec line 5632:
                // 'Walking-the-beat pattern; even spacing along
                // perimeter.' Linear gradient pulls the cat along the
                // patrol arc.
                Consideration::Spatial(SpatialConsideration::new(
                    "patrol_perimeter_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::TerritoryPerimeterAnchor),
                    PATROL_PERIMETER_RANGE,
                    Curve::Linear {
                        slope: -1.0,
                        intercept: 1.0,
                    },
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Dse for PatrolDse {
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
        // §7.3: Guarding → Blind. Territory defense shouldn't flinch
        // mid-patrol. AI8 caps fixation.
        CommitmentStrategy::Blind
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "territory_patrolled",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::Blind,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn patrol_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(PatrolDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrol_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(PatrolDse::new(&s).id().0, "patrol");
    }

    #[test]
    fn patrol_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            PatrolDse::new(&s).composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn patrol_has_four_considerations() {
        // §L2.10.7: deficit + boldness + safety_upper_bound +
        // perimeter_distance.
        let s = ScoringConstants::default();
        assert_eq!(PatrolDse::new(&s).considerations().len(), 4);
    }

    #[test]
    fn patrol_uses_territory_perimeter_anchor() {
        let s = ScoringConstants::default();
        let dse = PatrolDse::new(&s);
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "patrol_perimeter_distance" => Some(sp),
                _ => None,
            })
            .expect("patrol_perimeter_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::TerritoryPerimeterAnchor)
        ));
    }

    /// Helper: pull the scalar curve from a Consideration enum variant.
    /// Test-local — Patrol's first three considerations are scalars.
    fn scalar_curve(c: &Consideration) -> &Curve {
        match c {
            Consideration::Scalar(s) => &s.curve,
            _ => panic!("expected scalar consideration"),
        }
    }

    #[test]
    fn safety_upper_bound_curve_gates_above_exit_threshold() {
        // The third consideration's curve must output near-1 at low
        // safety and near-0 at high safety, with the transition
        // centered at `patrol_exit_threshold`. With steepness=20 and
        // default threshold 0.5, the transition is sharp.
        let s = ScoringConstants::default();
        let dse = PatrolDse::new(&s);
        let upper = scalar_curve(&dse.considerations()[2]);

        // Below threshold — gate is open (near-1).
        assert!(upper.evaluate(0.2) > 0.95);
        assert!(upper.evaluate(0.35) > 0.9);

        // At threshold — midpoint.
        assert!((upper.evaluate(0.5) - 0.5).abs() < 0.01);

        // Above threshold — gate closes (near-0).
        assert!(upper.evaluate(0.6) < 0.15);
        assert!(upper.evaluate(0.7) < 0.05);
        assert!(upper.evaluate(1.0) < 0.01);
    }

    #[test]
    fn patrol_score_near_zero_at_high_safety() {
        // End-to-end via per-axis evaluation: when safety has
        // recovered past the exit threshold, the upper-bound axis
        // gates the score toward zero. CompensatedProduct's
        // "zero-on-any-axis ⇒ zero output" property means Patrol's
        // composed score is effectively zero. This is the
        // loop-breaker that iter 2 ships.
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        let dse = PatrolDse::new(&s);

        let safety_high: f32 = 0.8;
        // Deficit axis evaluates `1 - safety`.
        let deficit_input = 1.0 - safety_high;
        // safety=0.8 → deficit=0.2 → Logistic(6,0.8)(0.2)≈0.027.
        let a0 = scalar_curve(&dse.considerations()[0]).evaluate(deficit_input);
        // Boldness axis evaluates the boldness scalar directly.
        let a1 = scalar_curve(&dse.considerations()[1]).evaluate(1.0);
        // Upper-bound axis evaluates `safety` directly.
        let a2 = scalar_curve(&dse.considerations()[2]).evaluate(safety_high);
        assert!(
            a2 < 0.05,
            "upper-bound gate must close at safety=0.8 (got {})",
            a2
        );
        // Sanity: deficit small, boldness fully open.
        assert!(a0 < 0.1);
        assert!(a1 > 0.9);

        // Sanity: at low safety all three axes are open.
        let safety_low: f32 = 0.15;
        let deficit_low = 1.0 - safety_low;
        let a0_low = scalar_curve(&dse.considerations()[0]).evaluate(deficit_low);
        let a1_low = scalar_curve(&dse.considerations()[1]).evaluate(1.0);
        let a2_low = scalar_curve(&dse.considerations()[2]).evaluate(safety_low);
        assert!(a0_low > 0.5);
        assert!(a1_low > 0.9);
        assert!(a2_low > 0.9);

        // Composition mode sanity.
        assert_eq!(dse.composition().mode, CompositionMode::CompensatedProduct);
    }
}
