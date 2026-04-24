//! `HuntTargetDse` — §6.5.5 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning prey selection. Pairs with the self-state
//! [`HuntDse`](super::hunt::hunt_dse) which decides *whether* to hunt;
//! this DSE decides *which prey*.
//!
//! Phase 4c.7 scope — yield-aware prey targeting:
//!
//! - `goap.rs::resolve_search_prey`'s `visible_prey.min_by_key(|...|
//!   pos.manhattan_distance(...))` picker retires for the visible-prey
//!   path. §6.1 Partial fix: the resolver today "picks `min_distance`
//!   regardless of yield," so a Mouse at range 5 is chosen over a
//!   Rabbit at range 7 even though the Rabbit delivers 1.3× food
//!   value. With the DSE, the Rabbit wins — assuming alertness hasn't
//!   swung the score.
//! - The scent-detection path (`scented_prey`) remains unchanged —
//!   scent resolves through the §5 influence-map source tile, and the
//!   single-target `min_by_key(source_distance)` is the geometry of
//!   the scent gradient, not a candidate-ranking choice.
//!
//! Three per-target considerations per §6.5.5. The `pursuit-cost` axis
//! is deferred pending §L2.10.7's plan-cost feedback shape — until
//! then, distance² (via `Quadratic(exp=2)` on nearness) stands in,
//! matching the spec's "pursuit-cost proxies as `distance²`" fallback
//! note. Weights renormalized from (0.25/0.25/0.20/0.30) by dropping
//! the 0.30 pursuit-cost row and dividing by 0.70:
//!
//! | # | Consideration          | Scalar name          | Curve                    | Spec weight | Renormalized |
//! |---|------------------------|----------------------|--------------------------|-------------|--------------|
//! | 1 | distance               | `target_nearness`    | `Quadratic(exp=2)`       | 0.25        | 0.357        |
//! | 2 | prey-species-yield     | `prey_yield`         | `Linear(1, 0)`           | 0.25        | 0.357        |
//! | 3 | prey-alertness (inv)   | `prey_calm`          | `Linear(1, 0)`           | 0.20        | 0.286        |
//!
//! **Yield normalization.** `ItemKind::food_value()` maxes at 0.8
//! (RawRat). The resolver divides by `YIELD_NORMALIZER = 0.8` so the
//! curve input lands in `[0, 1]` — the Linear(1, 0) curve is then a
//! pass-through in spec space. Rabbit (0.65) → 0.8125 normalized;
//! Mouse (0.5) → 0.625; Bird (0.6) → 0.75; Fish (0.7) → 0.875.
//!
//! **Alertness inversion.** `PreyState.alertness` is already `[0, 1]`
//! (from `prey_state.alertness`). The spec row specifies
//! `Linear(slope=-1, intercept=1)` — an inversion. The resolver feeds
//! `prey_calm = 1 - alertness` directly and keeps the curve as
//! `Linear(1, 0)` so the inversion lives in one place.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::components::prey::PreyKind;

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const PREY_YIELD_INPUT: &str = "prey_yield";
pub const PREY_CALM_INPUT: &str = "prey_calm";

/// Candidate-pool range in Manhattan tiles. Matches the cat sensory
/// profile's visual detection range (15) — the same outer gate that
/// `resolve_search_prey::visible_prey` uses today. Changing it would
/// shift the candidate population and is a balance decision deferred
/// to post-refactor per open-work #14.
pub const HUNT_TARGET_RANGE: f32 = 15.0;

/// Maximum `ItemKind::food_value()` across raw-prey variants (RawRat =
/// 0.8). Division by this normalizes the yield signal into `[0, 1]`
/// before the Linear curve evaluates.
pub const YIELD_NORMALIZER: f32 = 0.8;

/// Per-prey snapshot fed to `resolve_hunt_target`. Callers build a
/// `Vec<PreyCandidate>` from the frame-local prey query so the
/// resolver doesn't double-borrow it. `PreyKind` stays embedded so
/// the resolver can look up yield via the species food-value table
/// without another component query.
#[derive(Clone, Copy, Debug)]
pub struct PreyCandidate {
    pub entity: Entity,
    pub position: Position,
    pub kind: PreyKind,
    /// `PreyState.alertness` at snapshot time — already in `[0, 1]`.
    pub alertness: f32,
}

/// §6.5.5 `Hunt` target-taking DSE factory.
pub fn hunt_target_dse() -> TargetTakingDse {
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // Quadratic falloff — `target_nearness = 1 - dist/range`, squared.
    // Nearby prey dominate; distance contribution drops to ~0 by the
    // outer gate. Matches spec §6.4 row #5.
    let nearness_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };

    TargetTakingDse {
        id: DseId("hunt_target"),
        candidate_query: hunt_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(TARGET_NEARNESS_INPUT, nearness_curve)),
            Consideration::Scalar(ScalarConsideration::new(PREY_YIELD_INPUT, linear.clone())),
            Consideration::Scalar(ScalarConsideration::new(PREY_CALM_INPUT, linear)),
        ],
        // WeightedSum — matches the §6.1-Critical spec decision that
        // no axis should null a candidate (a loud rabbit is still
        // huntable). CompensatedProduct would gate alertness at 1.0,
        // and the spec explicitly wants alertness as a linear bias,
        // not a multiplicative lock-out.
        composition: Composition::weighted_sum(vec![0.357, 0.357, 0.286]),
        aggregation: TargetAggregation::Best,
        intention: hunt_intention,
    }
}

fn hunt_candidate_query_doc(_cat: Entity) -> &'static str {
    "prey within HUNT_TARGET_RANGE, visible to cat sensory profile"
}

fn hunt_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "prey_caught",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

/// Normalized yield signal from a `PreyKind`. Reads `ItemKind::food_value`
/// via the standard `PreyConfig::item_kind` mapping and divides by
/// `YIELD_NORMALIZER` so the Linear curve sees `[0, 1]`. Inlined here
/// (not in `PreyKind`) because the normalizer is a consideration-
/// specific concern — `food_value` already has a documented meaning
/// on its own.
pub fn prey_yield_normalized(kind: PreyKind) -> f32 {
    let raw = match kind {
        PreyKind::Mouse => crate::components::items::ItemKind::RawMouse.food_value(),
        PreyKind::Rat => crate::components::items::ItemKind::RawRat.food_value(),
        PreyKind::Rabbit => crate::components::items::ItemKind::RawRabbit.food_value(),
        PreyKind::Fish => crate::components::items::ItemKind::RawFish.food_value(),
        PreyKind::Bird => crate::components::items::ItemKind::RawBird.food_value(),
    };
    (raw / YIELD_NORMALIZER).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best visible prey for `cat` via the registered
/// [`hunt_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
///
/// `candidates` is the caller-built snapshot of in-sensory-range prey
/// (visible or scent-confirmed); it must already pass whatever species/
/// sensing filters the caller applies. The resolver does not re-filter
/// by range — it re-computes distance for the nearness axis, but trusts
/// the candidate list's eligibility.
pub fn resolve_hunt_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    candidates: &[PreyCandidate],
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "hunt_target")?;

    if candidates.is_empty() {
        return None;
    }

    // Pull entity + position parallel-vecs for the evaluator.
    let entities: Vec<Entity> = candidates.iter().map(|c| c.entity).collect();
    let positions: Vec<Position> = candidates.iter().map(|c| c.position).collect();

    // Lookup-table for the target fetchers. O(N) size; tiny — hunt
    // candidate pools are bounded by visual range × prey density.
    let kind_map: std::collections::HashMap<Entity, PreyKind> =
        candidates.iter().map(|c| (c.entity, c.kind)).collect();
    let alertness_map: std::collections::HashMap<Entity, f32> = candidates
        .iter()
        .map(|c| (c.entity, c.alertness.clamp(0.0, 1.0)))
        .collect();
    let pos_map: std::collections::HashMap<Entity, Position> = candidates
        .iter()
        .map(|c| (c.entity, c.position))
        .collect();

    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, _cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_NEARNESS_INPUT => {
                let target_pos = match pos_map.get(&target) {
                    Some(p) => *p,
                    None => return 0.0,
                };
                let dist = cat_pos.manhattan_distance(&target_pos) as f32;
                (1.0 - dist / HUNT_TARGET_RANGE).clamp(0.0, 1.0)
            }
            PREY_YIELD_INPUT => kind_map
                .get(&target)
                .copied()
                .map(prey_yield_normalized)
                .unwrap_or(0.0),
            PREY_CALM_INPUT => alertness_map
                .get(&target)
                .map(|a| 1.0 - a)
                .unwrap_or(0.5),
            _ => 0.0,
        }
    };

    let sample_map = |_: &str, _: Position| -> f32 { 0.0 };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat,
        tick,
        sample_map: &sample_map,
        has_marker: &has_marker,
        self_position: cat_pos,
        target: None,
        target_position: None,
    };

    let scored = evaluate_target_taking(
        dse,
        cat,
        &entities,
        &positions,
        &ctx,
        &fetch_self,
        &fetch_target,
    );

    // §11 focal-cat per-candidate ranking capture (§6.3). Emitted only
    // when the caller marks this resolve as the focal cat's tick.
    // Non-focal paths pass `focal_hook: None` and pay zero cost.
    if let Some(hook) = focal_hook {
        if let Some(ranking) = crate::ai::target_dse::target_ranking_from_scored(
            &scored,
            dse.aggregation(),
            hook.name_lookup,
        ) {
            hook.capture
                .set_target_ranking("hunt_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(entity_id: u32, x: i32, y: i32, kind: PreyKind, alertness: f32) -> PreyCandidate {
        PreyCandidate {
            entity: Entity::from_raw_u32(entity_id).unwrap(),
            position: Position::new(x, y),
            kind,
            alertness,
        }
    }

    #[test]
    fn hunt_target_dse_id_stable() {
        assert_eq!(hunt_target_dse().id().0, "hunt_target");
    }

    #[test]
    fn hunt_target_has_three_axes() {
        assert_eq!(hunt_target_dse().per_target_considerations().len(), 3);
    }

    #[test]
    fn hunt_target_weights_sum_to_one() {
        let sum: f32 = hunt_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn hunt_target_uses_best_aggregation() {
        assert_eq!(hunt_target_dse().aggregation(), TargetAggregation::Best);
    }

    #[test]
    fn intention_is_hunt_prey_goal() {
        let dse = hunt_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label, "prey_caught");
                assert_eq!(strategy, CommitmentStrategy::SingleMinded);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    #[test]
    fn prey_yield_normalization_respects_species_ranking() {
        // Ranking matches the food_value table: Rat > Fish > Rabbit
        // > Bird > Mouse. Normalized values land in [0, 1] with Rat
        // pinned to 1.0 (the normalizer).
        let rat = prey_yield_normalized(PreyKind::Rat);
        let fish = prey_yield_normalized(PreyKind::Fish);
        let rabbit = prey_yield_normalized(PreyKind::Rabbit);
        let bird = prey_yield_normalized(PreyKind::Bird);
        let mouse = prey_yield_normalized(PreyKind::Mouse);
        assert!((rat - 1.0).abs() < 1e-5);
        assert!(rat > fish);
        assert!(fish > rabbit);
        assert!(rabbit > bird);
        assert!(bird > mouse);
        assert!(mouse > 0.0);
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_hunt_target(&registry, cat, Position::new(0, 0), &[], 0, None);
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_with_empty_candidates() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(hunt_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_hunt_target(&registry, cat, Position::new(0, 0), &[], 0, None);
        assert!(out.is_none());
    }

    #[test]
    fn picks_higher_yield_at_equal_distance_and_alertness() {
        // §6.1 Partial fix demo: Rabbit (yield=0.8125) wins over Mouse
        // (yield=0.625) when distance and alertness are tied.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(hunt_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let mouse = candidate(2, 3, 0, PreyKind::Mouse, 0.2);
        let rabbit = candidate(3, 0, 3, PreyKind::Rabbit, 0.2);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[mouse, rabbit],
            0,
            None,
        );
        assert_eq!(out, Some(rabbit.entity));
    }

    #[test]
    fn alertness_penalizes_otherwise_better_prey() {
        // A very alert Rabbit (alertness=0.95, calm=0.05) loses to a
        // relaxed Mouse (alertness=0.0, calm=1.0) at the same distance.
        // Calc: Rabbit score = 1.0*0.357 + 0.8125*0.357 + 0.05*0.286
        //                    ≈ 0.357 + 0.290 + 0.014 = 0.661
        //       Mouse score  = 1.0*0.357 + 0.625*0.357 + 1.0*0.286
        //                    ≈ 0.357 + 0.223 + 0.286 = 0.866
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(hunt_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let alert_rabbit = candidate(2, 1, 0, PreyKind::Rabbit, 0.95);
        let relaxed_mouse = candidate(3, 0, 1, PreyKind::Mouse, 0.0);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[alert_rabbit, relaxed_mouse],
            0,
            None,
        );
        assert_eq!(out, Some(relaxed_mouse.entity));
    }

    #[test]
    fn close_prey_outscores_distant_same_species() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(hunt_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = candidate(2, 2, 0, PreyKind::Rabbit, 0.2);
        let far = candidate(3, 12, 0, PreyKind::Rabbit, 0.2);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[close, far],
            0,
            None,
        );
        assert_eq!(out, Some(close.entity));
    }

    #[test]
    fn distance_quadratic_penalty_dominates_small_yield_edge() {
        // A Rat (0.8 raw → 1.0 normalized, the richest prey) at
        // distance 10 loses to a Mouse (0.5 → 0.625 normalized) at
        // distance 1 because the quadratic nearness curve drops the
        // Rat's distance contribution to ~0.11 while the Mouse sits
        // near 0.87.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(hunt_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let near_mouse = candidate(2, 1, 0, PreyKind::Mouse, 0.1);
        let far_rat = candidate(3, 10, 0, PreyKind::Rat, 0.1);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[near_mouse, far_rat],
            0,
            None,
        );
        assert_eq!(out, Some(near_mouse.entity));
    }

    #[test]
    fn retires_min_distance_only_behavior() {
        // Exact §6.1-Partial scenario: at equal yield and alertness,
        // the DSE still picks by distance (matches legacy). The key
        // is that when yield differs, the tie-break is yield, not
        // iteration order — a Rabbit slightly farther than a Mouse
        // still wins when the quadratic nearness gap is smaller than
        // the yield gap. Verified by the higher-yield test above.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(hunt_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let near = candidate(2, 1, 0, PreyKind::Mouse, 0.2);
        let far = candidate(3, 5, 0, PreyKind::Mouse, 0.2);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[near, far],
            0,
            None,
        );
        assert_eq!(out, Some(near.entity));
    }
}
