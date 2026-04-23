//! `Flee` — Fatal-threat peer (§3.3.2 anchor = 1.0). Cross-species
//! peer of fox `Fleeing` through the flee-or-fight logistic anchor.
//!
//! Per §2.3 + §3.1.1: `CompensatedProduct` of two axes —
//! `safety_deficit` via `flee_or_fight(midpoint=flee_safety_threshold)`
//! and `boldness` via `Composite { Linear, Invert }`. Both gate:
//! bold cats never flee; fully safe cats have nothing to flee from.
//!
//! Maslow tier 2 — matches the inline `level_suppression(2)` factor
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
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{flee_or_fight, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::ai::faction::StanceRequirement;
use crate::resources::sim_constants::ScoringConstants;

pub const SAFETY_DEFICIT_INPUT: &str = "safety_deficit";
pub const BOLDNESS_INPUT: &str = "boldness";

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
        let boldness_invert = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };

        Self {
            id: DseId("flee"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SAFETY_DEFICIT_INPUT, safety_curve)),
                Consideration::Scalar(ScalarConsideration::new(BOLDNESS_INPUT, boldness_invert)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            // §9.3 DSE filter binding — Flee triggers on `Predator` stance.
            // §13.1: `.forbid("Incapacitated")` blocks downed cats — a
            // cat with an unhealed Severe injury can't flee, matching
            // the retired inline `if ctx.is_incapacitated` early-return.
            eligibility: EligibilityFilter::new()
                .with_stance(StanceRequirement::flee())
                .forbid("Incapacitated"),
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
    fn boldness_curve_inverts_input() {
        let s = ScoringConstants::default();
        let dse = FleeDse::new(&s);
        let c = match &dse.considerations()[1] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        // Invert: (1 - x), clamped.
        assert!((c.evaluate(0.0) - 1.0).abs() < 1e-4);
        assert!((c.evaluate(1.0) - 0.0).abs() < 1e-4);
        assert!((c.evaluate(0.5) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn bold_cat_produces_zero_flee_score() {
        use crate::ai::eval::{evaluate_single, ModifierPipeline};
        use crate::components::physical::Position;
        let s = ScoringConstants::default();
        let dse = FleeDse::new(&s);
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        // boldness = 1.0 → inverted = 0.0 → CP gate closes.
        let fetch = |name: &str, _: Entity| match name {
            "safety_deficit" => 0.9,
            "boldness" => 1.0,
            _ => 0.0,
        };
        let scored = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch)
            .expect("eligible");
        assert!(scored.raw_score < 0.01, "bold cat flees: {}", scored.raw_score);
    }
}
