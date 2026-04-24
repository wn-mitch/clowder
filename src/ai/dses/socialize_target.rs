//! `SocializeTargetDse` — §6.5.1 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning partner selection for `Socialize`. Pairs
//! with the self-state [`SocializeDse`](super::socialize::SocializeDse)
//! which owns the *desire* to socialize; this DSE owns *with whom*.
//!
//! Phase 4c.1 scope (silent-divergence-fix only):
//!
//! - The evaluator produces a `winning_target` that both
//!   `disposition.rs::build_socializing_chain` and
//!   `goap.rs::SocializeWith` consume. Replaces the divergent pickers
//!   at `disposition.rs:1348–1365` (fondness + novelty weighted sum)
//!   and `goap.rs:find_social_target` (fondness only).
//! - The `aggregated_score` is observable but does *not* modulate the
//!   `Action::Socialize` pool entry in this phase. The existing
//!   self-state [`SocializeDse`] continues to drive action selection;
//!   target-quality merging into the action pool is deferred until the
//!   scoring substrate's other §6 ports stabilize.
//!
//! Four per-target considerations per §6.5.1:
//!
//! | # | Consideration          | Scalar name             | Curve                           | Weight |
//! |---|------------------------|-------------------------|---------------------------------|--------|
//! | 1 | distance               | `target_nearness`       | `Quadratic(exp=2)` over range=8 | 0.25   |
//! | 2 | fondness               | `target_fondness`       | `Linear(1, 0)`                  | 0.35   |
//! | 3 | novelty (1-familiarity)| `target_novelty`        | `Linear(1, 0)`                  | 0.25   |
//! | 4 | species-compat         | `target_species_compat` | `Cliff(threshold=0.5)`          | 0.15   |
//!
//! Distance is encoded as a target-scoped scalar rather than a
//! `Spatial` consideration because no influence map represents
//! "distance from scoring cat to this specific candidate" — that's
//! point-to-point geometry, not a grid sample. The target-scoped
//! fetcher (`fetch_target_scalar(name, cat, target)`) has access to
//! both entities and can resolve the geometry via a captured position
//! snapshot.
//!
//! Novelty is stored pre-inverted (`1 - familiarity`) rather than via
//! a `Linear(-1, 1)` curve so the fetcher is the single source of
//! "novelty signal", matching the spec's naming of `target_novelty`
//! as a first-class axis.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::resources::relationships::Relationships;

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";
pub const TARGET_NOVELTY_INPUT: &str = "target_novelty";
pub const TARGET_SPECIES_COMPAT_INPUT: &str = "target_species_compat";

/// Candidate-pool range in Manhattan tiles. Matches the existing
/// `DispositionConstants::social_target_range` outer-gate semantic —
/// changing it would shift the candidate population and is a balance
/// decision deferred to post-refactor per open-work #14.
pub const SOCIALIZE_TARGET_RANGE: f32 = 10.0;

/// §6.5.1 `Socialize` target-taking DSE factory. Produces a
/// [`TargetTakingDse`] consumable by `add_target_taking_dse`.
pub fn socialize_target_dse() -> TargetTakingDse {
    let nearness_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // Cliff at 0.5 — species-compat scored as 1.0 above threshold, 0.0
    // below. Today only cat-on-cat socializing exists (the caller's
    // candidate filter already enforces species); under future visitor
    // / cross-species mechanics the fetcher returns the compatibility
    // class and the cliff gates it.
    let species_cliff = Curve::Piecewise {
        knots: vec![(0.0, 0.0), (0.499, 0.0), (0.5, 1.0), (1.0, 1.0)],
    };

    TargetTakingDse {
        id: DseId("socialize_target"),
        candidate_query: socialize_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(TARGET_NEARNESS_INPUT, nearness_curve)),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_FONDNESS_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(TARGET_NOVELTY_INPUT, linear)),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_SPECIES_COMPAT_INPUT,
                species_cliff,
            )),
        ],
        // WeightedSum matches the pre-refactor resolver's linear mixer
        // (`fondness × w1 + (1 - familiarity) × w2`). CompensatedProduct
        // would gate any low axis (a 0.0 novelty nulls the candidate
        // entirely) which over-punishes familiar-but-beloved partners.
        composition: Composition::weighted_sum(vec![0.25, 0.35, 0.25, 0.15]),
        aggregation: TargetAggregation::Best,
        intention: socialize_intention,
    }
}

/// Documentation-only candidate-query stub per §6.3 (the runtime
/// evaluator takes candidates as a direct argument; this fn-pointer
/// names where the caller's `SystemParam` bundle pulls them from).
fn socialize_candidate_query_doc(_cat: bevy::prelude::Entity) -> &'static str {
    "cats within social_target_range, excluding self, filtered by faction Same|Ally"
}

/// Intention factory threading the winning target forward. §L2.10
/// activity per `ActivityKind::Socialize`. The target entity rides on
/// the ScoredTargetTakingDse's `winning_target` field — this factory
/// produces the Intention shape that the commitment layer consumes;
/// the target itself is carried alongside.
fn socialize_intention(_target: Entity) -> Intention {
    Intention::Activity {
        kind: ActivityKind::Socialize,
        termination: Termination::UntilInterrupt,
        strategy: CommitmentStrategy::OpenMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver — the single entry point both disposition.rs and
// goap.rs invoke to pick a social partner. Retires the divergent pickers
// at `disposition.rs:1348` (weighted) and `goap.rs:find_social_target`
// (fondness-only) per §6.2.
// ---------------------------------------------------------------------------

/// Pick the best social partner for `cat` via the registered
/// [`socialize_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
///
/// `cat_positions` must be the frame-local snapshot of `(Entity,
/// Position)` for alive cats that disposition.rs / goap.rs already
/// build for their scoring ticks — this helper does not query the
/// world directly.
///
/// Per §6.2 this is the **only** sanctioned target-picker for
/// Socialize. Callers that currently call `find_social_target` or
/// `build_socializing_chain`'s inline picker must switch to this
/// helper; the legacy helpers stay behind for `GroomOther` /
/// `MentorCat` / `MateWith` only until their §6.5.2–§6.5.4 ports land.
pub fn resolve_socialize_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    // Find the registered factory output. Fall back to `None` if
    // registration was skipped (tests / partial bootstraps) — callers
    // interpret that as "no target picked" and the outer eligibility
    // gate short-circuits the action.
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "socialize_target")?;

    // Gather candidates + positions, excluding self and out-of-range
    // peers. `SOCIALIZE_TARGET_RANGE` matches today's
    // `DispositionConstants::social_target_range` (10) to preserve
    // outer-gate semantics; per-target distance attenuation happens
    // inside the DSE via the `target_nearness` Quadratic.
    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    for (other, other_pos) in cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos) as f32;
        if dist <= SOCIALIZE_TARGET_RANGE {
            candidates.push(*other);
            positions.push(*other_pos);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Build an entity → position lookup for the target-scoped fetcher.
    // Parallel-vec lookup would be O(N) per consideration; the map is
    // O(1) per lookup at the cost of one small allocation per call.
    // Candidate pools are small (dozens at most within social range)
    // so allocation overhead is negligible.
    let pos_map: std::collections::HashMap<Entity, Position> = candidates
        .iter()
        .copied()
        .zip(positions.iter().copied())
        .collect();

    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_NEARNESS_INPUT => {
                let target_pos = match pos_map.get(&target) {
                    Some(p) => *p,
                    None => return 0.0,
                };
                let dist = cat_pos.manhattan_distance(&target_pos) as f32;
                (1.0 - dist / SOCIALIZE_TARGET_RANGE).clamp(0.0, 1.0)
            }
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
                .unwrap_or(0.0),
            TARGET_NOVELTY_INPUT => {
                let familiarity = relationships
                    .get(cat, target)
                    .map(|r| r.familiarity)
                    .unwrap_or(0.0);
                1.0 - familiarity
            }
            // Cat-on-cat social — candidate filter keeps this true today.
            // Future cross-species socializing routes a compatibility
            // class here; curve-cliff gates on ≥ 0.5.
            TARGET_SPECIES_COMPAT_INPUT => 1.0,
            _ => 0.0,
        }
    };

    // `sample_map` / `has_marker` are unused by Socialize's four scalar
    // considerations but required by `EvalCtx`. Stub with no-op closures.
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
        &candidates,
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
                .set_target_ranking("socialize_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::target_dse::evaluate_target_taking;
    use crate::components::physical::Position;
    use crate::ai::dse::EvalCtx;
    use bevy::prelude::Entity;

    fn test_ctx(entity: Entity) -> EvalCtx<'static> {
        static MARKER: fn(&str, Entity) -> bool = |_, _| false;
        static SAMPLE: fn(&str, Position) -> f32 = |_, _| 0.0;
        EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &SAMPLE,
            has_marker: &MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        }
    }

    #[test]
    fn socialize_target_dse_id_stable() {
        assert_eq!(socialize_target_dse().id().0, "socialize_target");
    }

    #[test]
    fn socialize_target_dse_has_four_axes() {
        assert_eq!(socialize_target_dse().per_target_considerations().len(), 4);
    }

    #[test]
    fn socialize_target_weights_sum_to_one() {
        let sum: f32 = socialize_target_dse()
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn socialize_target_uses_best_aggregation() {
        assert_eq!(
            socialize_target_dse().aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn picks_argmax_when_fondness_dominates() {
        // Two equally near, equally novel, equally cat candidates —
        // fondness decides the winner.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(10).unwrap();
        let acquaintance = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_NEARNESS_INPUT => 1.0,
                TARGET_FONDNESS_INPUT => {
                    if target == friend {
                        0.9
                    } else {
                        0.4
                    }
                }
                TARGET_NOVELTY_INPUT => 0.5,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(2, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[friend, acquaintance],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(friend));
    }

    #[test]
    fn retires_silent_divergence_vs_fondness_only() {
        // Legacy `find_social_target` picks by fondness alone. When
        // fondness is tied but novelty differs, the legacy path is
        // undefined (tie-break by iter order); the DSE picks the
        // novel partner, breaking the tie deterministically.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let novel_stranger = Entity::from_raw_u32(10).unwrap();
        let familiar_friend = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_NEARNESS_INPUT => 1.0,
                TARGET_FONDNESS_INPUT => 0.5, // tied
                TARGET_NOVELTY_INPUT => {
                    if target == novel_stranger {
                        0.9
                    } else {
                        0.1
                    }
                }
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(1, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[novel_stranger, familiar_friend],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(novel_stranger));
    }

    #[test]
    fn empty_candidates_yield_no_target() {
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |_: &str, _: Entity, _: Entity| 0.0;
        let out =
            evaluate_target_taking(&dse, cat, &[], &[], &ctx, &fetch_self, &fetch_target);
        assert!(out.winning_target.is_none());
        assert!(out.intention.is_none());
        assert_eq!(out.aggregated_score, 0.0);
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let other = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        let cat_positions = vec![(other, Position::new(1, 0))];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_when_no_candidates_in_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        // Position well beyond SOCIALIZE_TARGET_RANGE (10).
        let cat_positions = vec![(far, Position::new(50, 0))];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_picks_higher_fondness_all_else_equal() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(2).unwrap();
        let stranger = Entity::from_raw_u32(3).unwrap();

        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, friend).fondness = 0.8;
        relationships.get_or_insert(cat, friend).familiarity = 0.5;
        relationships.get_or_insert(cat, stranger).fondness = 0.1;
        relationships.get_or_insert(cat, stranger).familiarity = 0.5;

        // Both within range, at equal distance.
        let cat_positions = vec![
            (friend, Position::new(3, 0)),
            (stranger, Position::new(3, 1)),
        ];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        assert_eq!(out, Some(friend));
    }

    #[test]
    fn resolver_excludes_self() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        // Only self in the snapshot — must return None.
        let relationships = Relationships::default();
        let cat_positions = vec![(cat, Position::new(0, 0))];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_tiebreaks_fondness_ties_on_novelty() {
        // Exact §6.2 silent-divergence scenario: fondness tied across
        // candidates. Legacy `find_social_target` would tie-break by
        // iter order (nondeterministic); the DSE picks the novel
        // stranger deterministically via the novelty axis.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let familiar = Entity::from_raw_u32(2).unwrap();
        let novel = Entity::from_raw_u32(3).unwrap();

        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, familiar).fondness = 0.4;
        relationships.get_or_insert(cat, familiar).familiarity = 0.95;
        relationships.get_or_insert(cat, novel).fondness = 0.4;
        relationships.get_or_insert(cat, novel).familiarity = 0.05;

        let cat_positions = vec![
            (familiar, Position::new(3, 0)),
            (novel, Position::new(3, 1)),
        ];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        assert_eq!(out, Some(novel));
    }

    #[test]
    fn intention_is_socialize_activity() {
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let target = Entity::from_raw_u32(10).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |name: &str, _: Entity, _: Entity| -> f32 {
            match name {
                TARGET_NEARNESS_INPUT | TARGET_FONDNESS_INPUT | TARGET_NOVELTY_INPUT
                | TARGET_SPECIES_COMPAT_INPUT => 1.0,
                _ => 0.0,
            }
        };
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[target],
            &[Position::new(1, 0)],
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        match out.intention.expect("intention present for winning target") {
            Intention::Activity { kind, .. } => assert_eq!(kind, ActivityKind::Socialize),
            other => panic!("expected Activity intention, got {other:?}"),
        }
    }
}
