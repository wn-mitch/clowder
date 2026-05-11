//! `Flee` — Fatal-threat peer (§3.3.2 anchor = 1.0). Cross-species
//! peer of fox `Fleeing` through the flee-or-fight logistic anchor.
//!
//! Per §2.3 + §3.1.1: `CompensatedProduct` of two axes —
//! `safety_deficit` via `flee_or_fight(midpoint=flee_safety_threshold)`
//! and `boldness` via `Composite { Linear, Invert }`. Both gate:
//! bold cats never flee; fully safe cats have nothing to flee from.
//!
//! Maslow tier 2 — matches the inline `tier_suppression(2)` factor
//! in `scoring.rs`. Fleeing is a safety-layer response that a
//! starving cat should not pursue over eating.
//!
//! **Peer-group motivation.** Post-3c.1b, cat `Flee` still uses the
//! inline `(1-safety) × flee_safety_scale × (1-boldness) × l2`
//! formula with magnitude up to `flee_safety_scale ≈ 3.0` —
//! exceeding the 1.0 peer-group anchor. Porting it to CP with the
//! flee-or-fight Logistic compresses the peak to ~0.88, matching
//! fox `Fleeing` / `DenDefense` / cat `Fight`.
//!
//! **Eligibility gate.** `has_threat_nearby || safety < threshold`
//! stays as an outer gate in `score_actions`; §4 marker port lands
//! in Phase 3d.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{flee_or_fight, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::ai::faction::StanceRequirement;
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub const SAFETY_DEFICIT_INPUT: &str = "safety_deficit";
pub const BOLDNESS_INPUT: &str = "boldness";
/// Ticket 087 — interoceptive perception axis. `health_deficit`
/// gates Flee in a CompensatedProduct so wounded cats flee harder.
pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";

/// §L2.10.7 Flee range — Manhattan tiles for the nearest-threat
/// anchor. 12 ≈ a wildlife detection radius; outside this the
/// threat-distance signal is meaningless.
pub const FLEE_THREAT_RANGE: f32 = 12.0;

/// Ticket 087 — Logistic midpoint for the `health_deficit` axis on
/// Flee (and Sleep). Set to `1.0 - DispositionConstants::critical_health_threshold`
/// so the inflection lands at the same HP ratio that triggers the
/// disposition-layer panic interrupt — DSE scoring elevates Flee /
/// Rest at the same boundary the interrupt cares about, eliminating
/// the post-interrupt-replan churn (ticket 047 treadmill). 0.6
/// matches the default `critical_health_threshold = 0.4`; tunable
/// via `ScoringConstants::health_panic_midpoint` once that knob lands.
pub const HEALTH_PANIC_MIDPOINT: f32 = 0.6;

pub struct FleeDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FleeDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // §2.3: `Logistic(steepness=10, midpoint=flee_safety_threshold)`.
        // The midpoint is *threshold-form* (triggers flee when safety
        // drops below `flee_safety_threshold`). Reading the input as
        // safety_deficit = `1 - safety`, a midpoint at `threshold`
        // means the Logistic fires when deficit passes `threshold` —
        // semantically: "panic when safety drops to this level."
        let safety_curve = flee_or_fight(scoring.flee_safety_threshold);
        // Ticket 271 R1a — was `Composite { Linear(slope=1, intercept=0), Invert }`
        // which hard-zeros at boldness=1.0 and collapsed CP for the
        // bold-injured-cornered Mocha profile. The pre-inverted Linear
        // shape `slope=-0.5, intercept=1.0` floors at 0.5 for fully bold
        // cats while preserving "less bold ⇒ flees more" monotonicity.
        let boldness_invert = Curve::Linear {
            slope: -0.5,
            intercept: 1.0,
        };

        // §L2.10.7 row Flee: Power-Invert curve over distance to
        // nearest threat. Spec line 5630: 'Inverse-distance-from-
        // threat; closer threat is sharply more urgent.' At distance
        // 0 (cat on threat) → 1, half-range → 0.75, edge → 0. The
        // anchor is the threat *position* (NearestThreat), so closer
        // ↔ the cat is in immediate danger ↔ Flee fires hardest.
        let threat_distance = Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        };
        // Ticket 087 — `health_deficit` axis as a *bonus lift*, not a
        // gate. Linear `slope=0.4, intercept=0.6` floors the curve at
        // 0.6 (full health → 0.6 contribution) and saturates at 1.0
        // (full deficit → 1.0). This preserves CompensatedProduct
        // gating on the three pre-existing axes (a bold cat / safe cat
        // / distant-threat cat still scores Flee near zero) while
        // making wounded cats flee *harder* relative to healthy cats
        // under the same threat conditions.
        //
        // Logistic gating was tried first and rejected: with full-health
        // deficit = 0, the Logistic produces ~0.0025 and CP's geometric
        // mean drags healthy-cat-under-threat scores below Cook /
        // Wander — a cat at full HP can't flee, which is wrong.
        // Closes ticket 047's treadmill by making Flee win the
        // disposition contest at low HP instead of relying on the
        // post-interrupt panic-replan to find Flee on jitter.
        let health_curve = Curve::Linear {
            slope: 0.4,
            intercept: 0.6,
        };

        Self {
            id: DseId("flee"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SAFETY_DEFICIT_INPUT, safety_curve)),
                Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_invert)),
                Consideration::Spatial(SpatialConsideration::new(
                    "flee_threat_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestThreat),
                    FLEE_THREAT_RANGE,
                    threat_distance,
                )),
                Consideration::Scalar(ScalarConsideration::new(HEALTH_DEFICIT_INPUT, health_curve)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0, 1.0]),
            // §9.3 DSE filter binding — Flee triggers on `Predator` stance.
            // §13.1: `.forbid(markers::Incapacitated::KEY)` blocks downed cats — a
            // cat with an unhealed Severe injury can't flee, matching
            // the retired inline `if ctx.is_incapacitated` early-return.
            eligibility: EligibilityFilter::new()
                .with_stance(StanceRequirement::flee())
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Dse for FleeDse {
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
        // §7.5: Flee is the canonical Maslow-interrupt replacement.
        // Event-driven preemption installs a `Blind`-committed Flee so
        // it cannot itself be preempted by normal scoring until the
        // achievement condition (safety restored) fires.
        CommitmentStrategy::Blind
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "fled_to_safety",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::Blind,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn flee_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(FleeDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::considerations::LandmarkAnchor;

    #[test]
    fn flee_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(FleeDse::new(&s).id().0, "flee");
    }

    #[test]
    fn flee_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        let s = ScoringConstants::default();
        assert_eq!(
            FleeDse::new(&s).composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn flee_maslow_tier_is_two() {
        let s = ScoringConstants::default();
        assert_eq!(FleeDse::new(&s).maslow_tier(), 2);
    }

    #[test]
    fn flee_stance_requirement_is_predator_only() {
        use crate::ai::faction::FactionStance;
        let s = ScoringConstants::default();
        let req = FleeDse::new(&s)
            .eligibility()
            .required_stance
            .clone()
            .expect("§9.3 binding must populate required_stance");
        assert!(req.accepts(FactionStance::Predator));
        assert!(!req.accepts(FactionStance::Enemy));
        assert!(!req.accepts(FactionStance::Prey));
        assert!(!req.accepts(FactionStance::Same));
    }

    #[test]
    fn boldness_curve_floors_at_half_for_fully_bold_cats() {
        // Ticket 271 — was `Composite { Linear(slope=1, intercept=0), Invert }`
        // which produced a hard-zero at boldness=1.0 and collapsed
        // the CompensatedProduct geometric mean for bold-injured-
        // cornered profiles (Mocha: post-254 verification soak).
        // Pre-inverted Linear `slope=-0.5, intercept=1.0` preserves
        // the "less bold ⇒ flees more" monotonicity while flooring
        // at 0.5 so bold cats retain non-zero CP contribution.
        let s = ScoringConstants::default();
        let dse = FleeDse::new(&s);
        let c = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(sc) if sc.name == BOLDNESS_INPUT => Some(&sc.curve),
                _ => None,
            })
            .expect("boldness axis must exist");
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4, "timid cat → 1.0");
        assert!((c.evaluate(0.5) - 0.75).abs() < 1e-4, "mid-bold cat → 0.75");
        assert!((c.evaluate(1.0) - 0.5).abs() < 1e-4, "fully bold cat → 0.5 (was 0.0 pre-271)");
    }

    #[test]
    fn flee_uses_nearest_threat_anchor() {
        let s = ScoringConstants::default();
        let dse = FleeDse::new(&s);
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "flee_threat_distance" => Some(sp),
                _ => None,
            })
            .expect("flee_threat_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::NearestThreat)
        ));
        // Power-Invert: closer-threat = higher.
        assert!(spatial.curve.evaluate(0.0) > 0.99);
        assert!(spatial.curve.evaluate(1.0) < 0.01);
    }

    #[test]
    fn bold_cat_scores_flee_below_timid_cat_under_identical_threat() {
        // Ticket 271 — replaces the pre-271 `bold_cat_produces_zero_flee_score`
        // invariant. Pre-271, boldness=1.0 hard-zeroed the boldness-invert
        // axis and collapsed CP to ~0, which produced the soak failure
        // mode (bold-injured-cornered cats couldn't flee even when their
        // safety/threat axes saturated). Post-271 the invariant is
        // *relative*: a bold cat scores Flee LESS than a timid cat
        // under identical conditions, but the bold cat's score is no
        // longer collapsed to zero — leaving CP headroom for the
        // health-deficit axis and modifier lifts to elevate Flee when
        // genuinely needed.
        use crate::ai::eval::{evaluate_single, ModifierPipeline};
        use crate::components::physical::Position;
        let s = ScoringConstants::default();
        let dse = FleeDse::new(&s);
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |a: LandmarkAnchor| -> Option<Position> {
            match a {
                LandmarkAnchor::NearestThreat => Some(Position::new(0, 0)),
                _ => None,
            }
        };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
            field_cost: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let bold_fetch = |name: &str, _: Entity| match name {
            "safety_deficit" => 0.9,
            "boldness" => 1.0,
            _ => 0.0,
        };
        let timid_fetch = |name: &str, _: Entity| match name {
            "safety_deficit" => 0.9,
            "boldness" => 0.0,
            _ => 0.0,
        };
        let bold = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &bold_fetch)
            .expect("eligible");
        let timid = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &timid_fetch)
            .expect("eligible");
        assert!(
            bold.raw_score < timid.raw_score,
            "bold cat must score Flee below timid cat: bold={}, timid={}",
            bold.raw_score,
            timid.raw_score,
        );
        // Post-271: bold cat retains non-zero CP contribution
        // (floored at boldness-invert axis = 0.5).
        assert!(
            bold.raw_score > 0.4,
            "bold cat must retain non-zero Flee score post-271: {}",
            bold.raw_score,
        );
    }

    #[test]
    fn flee_has_four_axes_with_health() {
        // Ticket 087 — health_deficit axis added.
        let s = ScoringConstants::default();
        assert_eq!(FleeDse::new(&s).considerations().len(), 4);
    }

    #[test]
    fn flee_health_deficit_axis_floors_to_preserve_cp_gating() {
        // Ticket 087 — Linear `slope=0.4, intercept=0.6` so the curve
        // floors at 0.6 (full health) and saturates at 1.0 (full
        // deficit). Acts as a bonus lift on Flee, not a gate. CP
        // composition would crash a Logistic-shaped axis to ~0 at
        // full health, suppressing healthy-cat-under-threat scores;
        // the floor preserves the pre-existing three-axis gating
        // semantics while letting wounded cats outscore healthy cats
        // under identical threat conditions.
        let s = ScoringConstants::default();
        let dse = FleeDse::new(&s);
        let c = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(sc) if sc.name == HEALTH_DEFICIT_INPUT => Some(&sc.curve),
                _ => None,
            })
            .expect("health_deficit axis must exist");
        assert!((c.evaluate(0.0) - 0.6).abs() < 1e-4, "full health → 0.6");
        assert!((c.evaluate(0.5) - 0.8).abs() < 1e-4, "half deficit → 0.8");
        assert!((c.evaluate(1.0) - 1.0).abs() < 1e-4, "full deficit → 1.0");
    }

    #[test]
    fn wounded_cat_scores_flee_above_healthy_cat_under_threat() {
        // Ticket 087 — a wounded cat under threat must score Flee
        // *higher* than a healthy cat under the same threat. The
        // CompensatedProduct gate on health_deficit lifts the wounded
        // cat's product term while leaving the healthy cat's term
        // suppressed.
        use crate::ai::eval::{evaluate_single, ModifierPipeline};
        use crate::components::physical::Position;
        let s = ScoringConstants::default();
        let dse = FleeDse::new(&s);
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |a: LandmarkAnchor| -> Option<Position> {
            match a {
                LandmarkAnchor::NearestThreat => Some(Position::new(0, 0)),
                _ => None,
            }
        };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
            field_cost: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();

        let healthy_fetch = |name: &str, _: Entity| match name {
            "safety_deficit" => 0.7,
            "boldness" => 0.3,
            "health_deficit" => 0.0,
            _ => 0.0,
        };
        let wounded_fetch = |name: &str, _: Entity| match name {
            "safety_deficit" => 0.7,
            "boldness" => 0.3,
            "health_deficit" => 0.9,
            _ => 0.0,
        };
        let healthy = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &healthy_fetch)
            .expect("eligible");
        let wounded = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &wounded_fetch)
            .expect("eligible");
        assert!(
            wounded.raw_score > healthy.raw_score,
            "wounded cat must score Flee higher than healthy cat under same threat: \
             wounded={}, healthy={}",
            wounded.raw_score,
            healthy.raw_score,
        );
    }
}
