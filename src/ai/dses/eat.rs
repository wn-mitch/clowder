//! `Eat` — reference DSE for Phase 3b.2.
//!
//! The simplest possible port: single-consideration (hunger) on the
//! canonical §2.3 hangry anchor, `CompensatedProduct` composition,
//! Maslow tier 1. Serves as the template every Phase 3c fan-out
//! sub-agent mimics.
//!
//! Spec rows (fixed post-2026-04-21):
//!
//! | Axis | Shape | Rationale |
//! |---|---|---|
//! | §2.3 `Eat.hunger` | `Logistic(steepness=8, midpoint=0.75)` | Hangry anchor — threshold, not ramp. Every other hunger-axis DSE (Hunt, Forage, fox Hunting/Raiding) reuses this shape. |
//! | §3.1.1 `Eat` | `CompensatedProduct` | n=1 today; kept CP (not WS) so future axes (`food_available`, `digestion_gate`) compose with gating semantics. |
//! | §3.3.1 | RtM | Auto-derived from CP. |
//! | §L2.10.3 Intention | `Goal(hunger < threshold)` | Need-driven. |
//! | Maslow tier | 1 | Physiological survival — §3.4 pre-gate returns 1.0 at tier 1 regardless of context. |
//!
//! **Eligibility.** Today Eat is gated at the outer `score_actions`
//! level on `ctx.food_available`. The proper §4 port is
//! `.require("HasStoredFood")`; that authoring-system lands in Phase
//! 3d. For Phase 3b.2 the DSE ships with an empty `EligibilityFilter`
//! and the outer gate stays — the §4 port happens in 3c's fan-out
//! when the inline block retires.
//!
//! **Retired-constants note.** §2.3 does not list `eat_urgency_scale`
//! as retired (unlike `incapacitated_eat_urgency_*`), but once this
//! DSE fully replaces the inline arithmetic in Phase 3c,
//! `eat_urgency_scale` becomes dead weight. Follow-on cleanup can
//! delete it post-3c; tracked in the phase-3 balance doc.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::hangry;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

// ---------------------------------------------------------------------------
// Eat DSE
// ---------------------------------------------------------------------------

/// Scalar input name for the hunger consideration. The evaluator's
/// `fetch_scalar` closure resolves this against the cat's hunger
/// **urgency** (`1 - Needs.hunger` — spec §2.3 semantics), not the
/// raw `Needs.hunger` satiation scalar. See `scoring.rs::ctx_scalars`.
pub const HUNGER_INPUT: &str = "hunger_urgency";

/// Hunger threshold that satisfies the Eat goal. Below this, the cat
/// is sated; above, the goal state is unreached. 0.3 matches today's
/// `needs.rs` typical post-meal hunger drop.
pub const HUNGER_GOAL_THRESHOLD: f32 = 0.3;

pub struct EatDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl EatDse {
    pub fn new() -> Self {
        Self {
            id: DseId("eat"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                HUNGER_INPUT,
                hangry(),
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            // §4 marker eligibility (Phase 4b.2): the cat can only
            // score Eat if the colony has food in stores. Retires the
            // inline `if ctx.food_available` gate at
            // `scoring.rs::score_actions`. Populated by the caller-side
            // `MarkerSnapshot::set_colony("HasStoredFood", ...)`.
            eligibility: EligibilityFilter::new().require(markers::HasStoredFood::KEY),
        }
    }
}

impl Default for EatDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for EatDse {
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
        // §7.3: Eat is a constituent action of the Resting disposition
        // (`DispositionKind::constituent_actions`) and rides the Resting
        // class's `Blind` strategy. Physiological completion is the
        // only reason to drop mid-intention — the Maslow gate handles
        // preemption already; AI8 caps runaway holds.
        CommitmentStrategy::Blind
    }

    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "hunger_below_threshold",
                achieved: eat_goal_achieved,
            },
            strategy: CommitmentStrategy::Blind,
        }
    }

    fn maslow_tier(&self) -> u8 {
        1
    }
}

/// Goal predicate: hunger has dropped below the satiation threshold.
///
/// Reads `Needs.hunger` from the cat's components. When the component
/// is missing (happens in isolated unit tests that spawn a bare
/// entity), the predicate returns `false` — a missing needs component
/// is not "sated," it's undefined.
fn eat_goal_achieved(world: &World, cat: Entity) -> bool {
    world
        .get::<crate::components::physical::Needs>(cat)
        .is_some_and(|needs| needs.hunger < HUNGER_GOAL_THRESHOLD)
}

// ---------------------------------------------------------------------------
// Public constructor for registration
// ---------------------------------------------------------------------------

/// Build the Eat DSE for registration. Called once at plugin load.
pub fn eat_dse() -> Box<dyn Dse> {
    Box::new(EatDse::new())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn eat_dse_id_is_stable() {
        let dse = EatDse::new();
        assert_eq!(dse.id().0, "eat");
    }

    #[test]
    fn eat_dse_is_compensated_product() {
        let dse = EatDse::new();
        assert_eq!(
            dse.composition().mode,
            crate::ai::composition::CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn eat_dse_uses_hangry_anchor() {
        // Phase 3b.2 spec row: hunger via Logistic(8, 0.75). At
        // hunger=0.75 the score should be ~0.5 (logistic midpoint);
        // at hunger=0.9 it should be >0.85; at hunger=0.1 it should
        // be <0.01.
        let dse = EatDse::new();
        let entity = Entity::from_raw_u32(1).unwrap();

        // §4 (Phase 4b.2): Eat requires `HasStoredFood` — the test
        // closure stands in for the `MarkerSnapshot.has()` lookup
        // by returning true for that key.
        let has_marker = |name: &str, _: Entity| name == markers::HasStoredFood::KEY;
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

        let score = |hunger: f32| {
            let fetch = move |name: &str, _: Entity| {
                if name == HUNGER_INPUT {
                    hunger
                } else {
                    0.0
                }
            };
            evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch)
                .expect("eligible with HasStoredFood")
                .final_score
        };

        // Logistic(8, 0.75):
        //   x=0.75 → 0.5 (midpoint, exact)
        //   x=0.9  → 1/(1+e^{-1.2}) ≈ 0.77
        //   x=0.1  → 1/(1+e^{5.2}) ≈ 0.0055
        assert!(approx(score(0.75), 0.5, 0.01), "midpoint: {}", score(0.75));
        assert!(score(0.9) > 0.7, "starving: {}", score(0.9));
        assert!(score(0.1) < 0.01, "sated: {}", score(0.1));
    }

    #[test]
    fn eat_dse_maslow_tier_is_one() {
        let dse = EatDse::new();
        assert_eq!(dse.maslow_tier(), 1);
    }

    #[test]
    fn eat_dse_emits_goal_intention() {
        let dse = EatDse::new();
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
        let intention = dse.emit(0.5, &ctx);
        assert!(intention.is_goal(), "expected Goal intention");
        assert_eq!(intention.strategy(), CommitmentStrategy::Blind);
    }

    #[test]
    fn eat_goal_achieved_below_threshold() {
        let mut world = World::new();
        let entity = {
            let mut needs = crate::components::physical::Needs::default();
            needs.hunger = HUNGER_GOAL_THRESHOLD - 0.05;
            world.spawn(needs).id()
        };
        assert!(eat_goal_achieved(&world, entity));
    }

    #[test]
    fn eat_goal_not_achieved_above_threshold() {
        let mut world = World::new();
        let entity = {
            let mut needs = crate::components::physical::Needs::default();
            needs.hunger = 0.8;
            world.spawn(needs).id()
        };
        assert!(!eat_goal_achieved(&world, entity));
    }

    #[test]
    fn eat_goal_missing_needs_returns_false() {
        let mut world = World::new();
        let entity = world.spawn(()).id();
        assert!(!eat_goal_achieved(&world, entity));
    }

    #[test]
    fn eat_dse_requires_has_stored_food() {
        // Phase 4b.2: Eat's outer `ctx.food_available` gate in
        // `score_actions` retired; the DSE now consumes the
        // `HasStoredFood` colony marker via its eligibility filter.
        let dse = EatDse::new();
        assert_eq!(dse.eligibility().required, vec![markers::HasStoredFood::KEY]);
        assert!(dse.eligibility().forbidden.is_empty());
    }

    #[test]
    fn eat_dse_rejected_without_has_stored_food_marker() {
        // Inverse of the `uses_hangry_anchor` test: with the marker
        // absent, the evaluator must skip Eat entirely per §4's
        // "avoid computing a score that can't win" principle.
        let dse = EatDse::new();
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
        let fetch = |_: &str, _: Entity| 0.8_f32;

        let out = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch);
        assert!(out.is_none(), "Eat must be ineligible without HasStoredFood");
    }

    #[test]
    fn eat_dse_boxed_registers() {
        let registry_entry = eat_dse();
        assert_eq!(registry_entry.id().0, "eat");
    }
}
