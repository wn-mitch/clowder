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
    Consideration, FieldConsideration, FieldSource, LandmarkAnchor, LandmarkSource,
    ScalarConsideration, SpatialConsideration,
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
/// 263 — `LocationBeliefs.recency_of_threat_cue` at the cat's patrol
/// perimeter anchor bucket. Surfaced by `ctx_scalars` (precomputed
/// at `ScoringContext` construction so the consideration closure
/// doesn't need to thread a per-cat Component query).
pub const PATROL_THREAT_RECENCY_INPUT: &str = "patrol_threat_recency";

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
        let mut considerations: Vec<Consideration> = vec![
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
        ];
        let mut weights = vec![1.0_f32, 1.0, 1.0, 1.0];

        // 228: destination-aware refinement of 209's pattern. Reads
        // route cost (cat perception of "how hard to reach the
        // patrol perimeter") at TerritoryPerimeterAnchor. Curve
        // `Composite{Logistic(6.0, 0.4), Invert}` — high route cost
        // → low axis → CP gate suppresses Patrol in proportion to
        // the *real* path cost (terrain + danger + per-cat caution),
        // not just the cat's current cell. Conditionally added at
        // non-zero weight: CP semantics `(c · 0) = 0` would zero the
        // product if added at weight 0, so the axis is only present
        // when balance-tuning lifts the weight. Replaces 209's
        // `fox_scent_level` cat-position scalar (the v2 stopgap) per
        // ticket 228 v3 reframe — the `patrol.rs:108` comment that
        // reserved this slot lands here.
        let route_cost_weight = scoring.patrol_route_cost_weight.clamp(0.0, 1.0);
        if route_cost_weight > 0.0 {
            considerations.push(Consideration::Field(FieldConsideration::new(
                "patrol_route_cost",
                FieldSource::OwnRouteCost,
                LandmarkSource::Anchor(LandmarkAnchor::TerritoryPerimeterAnchor),
                PATROL_PERIMETER_RANGE,
                Curve::Composite {
                    inner: Box::new(Curve::Logistic {
                        steepness: 6.0,
                        midpoint: 0.4,
                    }),
                    post: PostOp::Invert,
                },
            )));
            weights.push(route_cost_weight);
        }

        // 263: conditional 6th axis `patrol_threat_recency` reads the
        // cat's per-location subjective belief facet at the patrol
        // perimeter anchor bucket (258 substrate). High recency_of_
        // threat_cue → low patrol attractiveness; the Linear-Invert
        // curve over the already-normalized `[0,1]` facet value keeps
        // the DSE-side shape neutral (the integrator's EMA is the
        // canonical shaping site). Conditionally added because CP
        // semantics `c · 0 = 0` would zero the whole product if
        // pushed at weight 0; ships dormant at the default
        // `patrol_threat_recency_weight = 0.0`. Activation in a
        // follow-on with the four-artifact methodology — this axis
        // is the substrate-side fix for the L3 patrol-absorption
        // cascade (`project_l3_patrol_absorption_cascade`), so its
        // landing needs a per-axis hypothesis-and-soak.
        let threat_recency_weight = scoring.patrol_threat_recency_weight.clamp(0.0, 1.0);
        if threat_recency_weight > 0.0 {
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                PATROL_THREAT_RECENCY_INPUT,
                Curve::Composite {
                    inner: Box::new(Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    }),
                    post: PostOp::Invert,
                },
            )));
            weights.push(threat_recency_weight);
        }

        Self {
            id: DseId("patrol"),
            considerations,
            composition: Composition::compensated_product(weights),
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
impl crate::ai::dse::CatDse for PatrolDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Patrol
    }
}

pub fn patrol_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
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
    fn patrol_has_five_considerations_at_default() {
        // §L2.10.7 + 256 R4: deficit + boldness + safety_upper_bound +
        // perimeter_distance + (256) patrol_route_cost. The fifth axis
        // is conditional on `patrol_route_cost_weight > 0`; ticket 256
        // activates the gate (default 0.6).
        let s = ScoringConstants::default();
        assert_eq!(PatrolDse::new(&s).considerations().len(), 5);
    }

    #[test]
    fn patrol_route_cost_active_at_default_post_256() {
        // 256 R4: `patrol_route_cost_weight` activated (default 0.6)
        // — was 0.0 in 228, dormant pending the L3 patrol-cascade
        // root-cause fix. The fifth axis (`patrol_route_cost` Field
        // consideration) now ships present at default; the L2
        // composition gates Patrol's score on path tractability.
        let s = ScoringConstants::default();
        assert!(s.patrol_route_cost_weight > 0.0, "active post-256");
        let dse = PatrolDse::new(&s);
        assert_eq!(dse.considerations().len(), 5);
        assert_eq!(dse.composition().weights.len(), 5);
        let has_axis = dse.considerations().iter().any(|c| match c {
            Consideration::Field(f) => f.name == "patrol_route_cost",
            _ => false,
        });
        assert!(has_axis, "patrol_route_cost axis present at default");
    }

    #[test]
    fn patrol_route_cost_dormant_when_weight_zeroed() {
        // Symmetric: when a balance experiment zeroes the weight,
        // the axis is omitted from the composition. CP semantics
        // make a weight-0 axis multiplicatively zero the product,
        // so dormancy requires axis omission, not just zero weight.
        let mut s = ScoringConstants::default();
        s.patrol_route_cost_weight = 0.0;
        let dse = PatrolDse::new(&s);
        assert_eq!(dse.considerations().len(), 4);
        assert_eq!(dse.composition().weights.len(), 4);
        assert!(!dse.considerations().iter().any(|c| match c {
            Consideration::Field(f) => f.name == "patrol_route_cost",
            _ => false,
        }));
    }

    #[test]
    fn patrol_threat_recency_axis_dormant_at_default() {
        // 263: `patrol_threat_recency_weight` ships dormant at 0.0; the
        // 6th axis MUST NOT appear in considerations (CP semantics
        // `c · 0 = 0` would zero the whole product).
        let s = ScoringConstants::default();
        assert_eq!(s.patrol_threat_recency_weight, 0.0);
        let dse = PatrolDse::new(&s);
        assert!(
            dse.considerations().iter().all(|c| !matches!(
                c,
                Consideration::Scalar(sc) if sc.name == PATROL_THREAT_RECENCY_INPUT
            )),
            "patrol_threat_recency axis must be absent at dormant weight"
        );
    }

    #[test]
    fn patrol_threat_recency_axis_present_when_weight_lifted() {
        // 263: when the activation follow-on lifts the weight, the 6th
        // axis appears as a Linear-Invert scalar and the CP picks up
        // its weight. High recency_of_threat_cue → low patrol score.
        let mut s = ScoringConstants::default();
        s.patrol_threat_recency_weight = 1.0;
        let dse = PatrolDse::new(&s);
        // Default config has patrol_route_cost active at 0.6, so the
        // pre-263 baseline is 5 axes — the new axis brings it to 6.
        assert_eq!(dse.considerations().len(), 6);
        assert_eq!(dse.composition().weights.len(), 6);
        assert!((dse.composition().weights[5] - 1.0).abs() < 1e-4);
        let curve = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(sc) if sc.name == PATROL_THREAT_RECENCY_INPUT => {
                    Some(&sc.curve)
                }
                _ => None,
            })
            .expect("patrol_threat_recency axis must exist at non-zero weight");
        // Linear-Invert over [0,1]: no threat memory → 1.0; saturated
        // → 0.0. Patrol score scales accordingly.
        assert!((curve.evaluate(0.0) - 1.0).abs() < 1e-4, "no threat → 1.0");
        assert!((curve.evaluate(1.0) - 0.0).abs() < 1e-4, "saturated → 0.0");
    }

    #[test]
    fn patrol_route_cost_axis_added_when_weight_nonzero() {
        // Symmetric: when balance-tuning lifts the weight, the axis
        // appears as the fifth consideration with the configured
        // weight. This is the substrate that 228 ships; tuning is a
        // follow-on ticket.
        let mut s = ScoringConstants::default();
        s.patrol_route_cost_weight = 0.3;
        let dse = PatrolDse::new(&s);
        assert_eq!(dse.considerations().len(), 5);
        assert_eq!(dse.composition().weights.len(), 5);
        assert!((dse.composition().weights[4] - 0.3).abs() < 1e-4);
        let has_axis = dse.considerations().iter().any(|c| match c {
            Consideration::Field(f) => f.name == "patrol_route_cost",
            _ => false,
        });
        assert!(has_axis);
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

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static PATROL_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 1400,
        construct: |s| patrol_dse(s),
    };
