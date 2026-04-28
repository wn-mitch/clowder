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
//! | §2.3 `Eat.hunger` | `Logistic(steepness=8, midpoint=0.5)` | Hangry anchor — recalibrated 2026-04-27 (ticket 044) from midpoint 0.75 to 0.5; cats now meaningfully consider food at "half-hungry" instead of waiting for emergency hunger. Every other hunger-axis DSE (Hunt, Forage, fox Hunting/Raiding) reuses this shape. |
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
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{hangry, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

/// Manhattan range over which the stores-distance curve is normalized
/// for Eat. Beyond this the curve saturates near zero. 20 tiles ≈ a
/// long colony walk; cats farther rarely commute solely to Eat.
pub const EAT_STORES_RANGE: f32 = 20.0;

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
        // §L2.10.7 spatial axis: distance to nearest stores tile
        // resolved via ColonyLandmarks. `Composite { Logistic(8, 0.5),
        // Invert }` gives `1 - Logistic(cost)` over normalized cost —
        // close-enough plateau, distant food viable but discounted
        // (spec rationale at considerations.rs:73). Outer
        // ClampMin(0.1) floor so distant cats still score non-zero
        // under CP composition; HasStoredFood marker still gates
        // entirely when the colony has no food.
        let stores_distance = Curve::Composite {
            inner: Box::new(Curve::Composite {
                inner: Box::new(Curve::Logistic {
                    steepness: 8.0,
                    midpoint: 0.5,
                }),
                post: PostOp::Invert,
            }),
            post: PostOp::ClampMin(0.1),
        };
        Self {
            id: DseId("eat"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_INPUT, hangry())),
                // §L2.10.7 spatial axis. Multiplicative with hunger
                // urgency under CompensatedProduct: starving cat at
                // stores ≈ 0.98 × hunger; starving cat 20 tiles away
                // ≈ 0.02 × hunger — discounted but not gated. The
                // marker-eligibility check on `HasStoredFood` still
                // gates the DSE entirely when no stores exist.
                Consideration::Spatial(SpatialConsideration::new(
                    "eat_stores_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestStores),
                    EAT_STORES_RANGE,
                    stores_distance,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]),
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
    use crate::ai::considerations::LandmarkAnchor;
    use super::*;
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

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
        // Recalibrated 2026-04-27 (ticket 044): hunger via Logistic(8, 0.5).
        // At urgency=0.5 (half-hungry) the score is ~0.5 (logistic midpoint);
        // at urgency=0.9 (starving) it should be >0.95; at urgency=0.1
        // (sated) it should be <0.05.
        let dse = EatDse::new();
        let entity = Entity::from_raw_u32(1).unwrap();

        // §4 (Phase 4b.2): Eat requires `HasStoredFood` — the test
        // closure stands in for the `MarkerSnapshot.has()` lookup
        // by returning true for that key.
        let has_marker = |name: &str, _: Entity| name == markers::HasStoredFood::KEY;
        let entity_position = |_: Entity| -> Option<Position> { None };
        // Place the stores at the cat's position so the §L2.10.7
        // spatial axis evaluates to ~0.98 (closest cost) — preserves
        // the hunger-anchor shape under CompensatedProduct in this
        // test; spatial-axis behavior is exercised separately.
        let anchor_position = |a: LandmarkAnchor| -> Option<Position> {
            match a {
                LandmarkAnchor::NearestStores => Some(Position::new(0, 0)),
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

        // Hunger axis Logistic(8, 0.5) drives the underlying shape.
        // Post-§L2.10.7 the DSE composes hunger × spatial under
        // CompensatedProduct, which lifts low-hunger scores via the
        // geometric-mean compensation (§3.2 strength=0.75). Bounds
        // accommodate the lift while still asserting the hunger
        // axis is monotonic and dominant:
        //   hunger=0.1 (sated)   → CP ≈ 0.26 (raw 0.098 · spatial 0.98)
        //   hunger=0.5 (midpoint) → CP ≈ 0.65
        //   hunger=0.9 (starving) → CP ≈ 0.93
        assert!(score(0.5) > 0.6 && score(0.5) < 0.7, "midpoint: {}", score(0.5));
        assert!(score(0.9) > 0.9, "starving: {}", score(0.9));
        assert!(score(0.1) < 0.3, "sated: {}", score(0.1));
        // Monotonicity check — CP's compensation must not invert the
        // hunger ordering.
        assert!(score(0.9) > score(0.5));
        assert!(score(0.5) > score(0.1));
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
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
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
        assert_eq!(
            dse.eligibility().required,
            vec![markers::HasStoredFood::KEY]
        );
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
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.8_f32;

        let out = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch);
        assert!(
            out.is_none(),
            "Eat must be ineligible without HasStoredFood"
        );
    }

    #[test]
    fn eat_dse_boxed_registers() {
        let registry_entry = eat_dse();
        assert_eq!(registry_entry.id().0, "eat");
    }
}
