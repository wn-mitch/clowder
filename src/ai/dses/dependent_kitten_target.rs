//! Ticket 364 — `DependentKittenTargetDse`. Pairs with the `rear_kitten`
//! HTN method's three primitive leaves (Wean / Teach / Release) registered
//! in `src/ai/methods/rear_kitten.rs`. The HTN dispatch closure pins
//! `chosen_action` to the leaf primitive at the L2 author site; this DSE
//! resolves the *target kitten* the leaf acts on.
//!
//! One factory parameterized by [`Action`] produces three sibling
//! `TargetTakingDse`s — same scoring shape, different DseId + eligibility
//! band:
//!
//! | Action     | Eligibility band                                      |
//! |------------|-------------------------------------------------------|
//! | `Wean`     | `maturity < weaned_threshold`                         |
//! | `Teach`    | `weaned_threshold <= maturity < teach_done_threshold` |
//! | `Release`  | `maturity >= teach_done_threshold`                    |
//!
//! All three filter candidates to `KittenDependency.mother == Some(self)`
//! per #333 §Out-of-scope (father-side rearing deferred). The leaves are
//! mutually exclusive on maturity, so at most one DSE fires per kitten per
//! tick.
//!
//! **Scoring axes (single).** Spatial nearness only — `Quadratic(exp=1.5,
//! divisor=-1, shift=1)` over normalized cost, matching the §L2.10.7
//! explicit-inversion idiom shared by caretake / mentor / socialize. The
//! maturity gate is a binary in/out-of-band filter (eligibility), not a
//! score axis — there's no graded preference between "barely in band" and
//! "deep in band" inside a single sub-goal.
//!
//! **Composition.** `WeightedSum` with the single nearness axis at weight
//! 1.0 — semantically equivalent to argmin distance among eligible
//! candidates, but routed through the IAUS pipeline so the trace surface
//! records the score in the standard target-DSE shape.
//!
//! **What this DSE does not own.** The leaf primitive's *action* — that's
//! pinned by the HTN frame at the L2 author site (commit b of #364). This
//! DSE only resolves *which kitten*.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, SpatialConsideration,
};
use crate::ai::curves::Curve;
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::ai::Action;
use crate::components::physical::Position;

/// Candidate-pool range in Manhattan tiles. Mirrors caretake (12) — queens
/// cross the colony for a dependent kitten in any rearing sub-goal.
pub const DEPENDENT_KITTEN_TARGET_RANGE: f32 = 12.0;

/// Minimal per-candidate kitten record consumed by
/// [`resolve_dependent_kitten_target`]. The dispatch arm at `goap.rs`
/// builds the slice once per call from `ExecutorContext::kitten_parentage`
/// + the cat-position snapshot.
#[derive(Debug, Clone, Copy)]
pub struct DependentKittenState {
    pub entity: Entity,
    pub pos: Position,
    pub maturity: f32,
    pub mother: Option<Entity>,
}

/// `rear_kitten` target-taking DSE factory parameterized by the primitive
/// leaf's action. Wean / Teach / Release each get a distinct registration;
/// see [`dependent_kitten_target_dse_id_for`] for the DseId convention.
///
/// # Panics
/// Panics if `action` is not one of `Wean` / `Teach` / `Release`. The HTN
/// method's sub-goal table is the only authoritative source of supported
/// actions; mismatched callers are programmer errors, not runtime cases.
pub fn dependent_kitten_target_dse(action: Action) -> TargetTakingDse {
    let id = dependent_kitten_target_dse_id_for(action);

    // §L2.10.7 distance axis: `(1 - cost)^1.5` via `Quadratic(exp=1.5,
    // divisor=-1, shift=1)`. Same explicit-inversion idiom as caretake.
    let nearness_curve = Curve::Quadratic {
        exponent: 1.5,
        divisor: -1.0,
        shift: 1.0,
    };

    TargetTakingDse {
        id,
        candidate_query: dependent_kitten_candidate_query_doc,
        per_target_considerations: vec![Consideration::Spatial(SpatialConsideration::new(
            "dependent_kitten_target_nearness",
            LandmarkSource::TargetPosition,
            DEPENDENT_KITTEN_TARGET_RANGE,
            nearness_curve,
        ))],
        composition: Composition::weighted_sum(vec![1.0]),
        aggregation: TargetAggregation::Best,
        intention: dependent_kitten_intention_for(action),
        required_stance: None,
        // 074 + 080 — gate dead / banished / reserved candidates. Maturity
        // band + mother-side filter is applied in the caller-side resolver
        // (the DSE-level `eligibility` field is consulted by the IAUS pass
        // only).
        eligibility: crate::systems::plan_substrate::require_alive_and_unreserved_filter(),
    }
}

/// Stable [`DseId`] convention for the three sibling registrations.
pub fn dependent_kitten_target_dse_id_for(action: Action) -> DseId {
    match action {
        Action::Wean => DseId("dependent_kitten_wean_target"),
        Action::Teach => DseId("dependent_kitten_teach_target"),
        Action::Release => DseId("dependent_kitten_release_target"),
        other => panic!("dependent_kitten_target_dse: unsupported action {other:?}"),
    }
}

fn dependent_kitten_candidate_query_doc(_cat: Entity) -> &'static str {
    "kittens with KittenDependency.mother == Some(self) within DEPENDENT_KITTEN_TARGET_RANGE and matching the action's maturity band"
}

fn dependent_kitten_intention_for(action: Action) -> fn(Entity) -> Intention {
    match action {
        Action::Wean => wean_intention,
        Action::Teach => teach_intention,
        Action::Release => release_intention,
        other => panic!("dependent_kitten_target_dse: unsupported action {other:?}"),
    }
}

fn wean_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "wean_kitten",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

fn teach_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "teach_kitten",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

fn release_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "release_kitten",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best dependent-kitten target for `queen` given the leaf
/// primitive's `action`. Returns `None` iff no kitten satisfies the
/// action's maturity band AND the mother-side filter AND the range gate.
///
/// `kittens` is built upstream by the dispatch arm from
/// `ExecutorContext::kitten_parentage` joined against the cat-position
/// snapshot. Kittens without a recorded position are excluded by the
/// caller (cannot score nearness).
#[allow(clippy::too_many_arguments)]
pub fn resolve_dependent_kitten_target(
    action: Action,
    registry: &DseRegistry,
    queen: Entity,
    queen_pos: Position,
    kittens: &[DependentKittenState],
    weaned_threshold: f32,
    teach_done_threshold: f32,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse_id = dependent_kitten_target_dse_id_for(action);
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == dse_id.0)?;

    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    for kitten in kittens {
        if kitten.mother != Some(queen) {
            continue;
        }
        if !maturity_in_band(action, kitten.maturity, weaned_threshold, teach_done_threshold) {
            continue;
        }
        let dist = queen_pos.manhattan_distance(&kitten.pos) as f32;
        if dist > DEPENDENT_KITTEN_TARGET_RANGE {
            continue;
        }
        candidates.push(kitten.entity);
        positions.push(kitten.pos);
    }

    if candidates.is_empty() {
        return None;
    }

    let fetch_self = |_name: &str, _queen: Entity| -> f32 { 0.0 };
    let fetch_target = |_name: &str, _queen: Entity, _target: Entity| -> f32 { 0.0 };

    let entity_position = |_: Entity| -> Option<Position> { None };
    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat: queen,
        tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
        has_marker: &has_marker,
        self_position: queen_pos,
        target: None,
        target_position: None,
        target_alive: None,
        field_cost: None,
    };

    let scored = evaluate_target_taking(
        dse,
        queen,
        &candidates,
        &positions,
        &ctx,
        &fetch_self,
        &fetch_target,
    );

    if let Some(hook) = focal_hook {
        if let Some(ranking) = crate::ai::target_dse::target_ranking_from_scored(
            &scored,
            dse.aggregation(),
            hook.name_lookup,
        ) {
            hook.capture
                .set_target_ranking(dse_id.0, ranking, tick);
        }
    }

    scored.winning_target
}

/// Per-action maturity-band predicate.
pub fn maturity_in_band(
    action: Action,
    maturity: f32,
    weaned_threshold: f32,
    teach_done_threshold: f32,
) -> bool {
    match action {
        Action::Wean => maturity < weaned_threshold,
        Action::Teach => maturity >= weaned_threshold && maturity < teach_done_threshold,
        Action::Release => maturity >= teach_done_threshold,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kitten(id: u32, x: i32, y: i32, maturity: f32, mother: Option<Entity>) -> DependentKittenState {
        DependentKittenState {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Position::new(x, y),
            maturity,
            mother,
        }
    }

    // -- Factory shape --------------------------------------------------------

    #[test]
    fn dse_id_per_action_is_stable() {
        assert_eq!(
            dependent_kitten_target_dse(Action::Wean).id().0,
            "dependent_kitten_wean_target"
        );
        assert_eq!(
            dependent_kitten_target_dse(Action::Teach).id().0,
            "dependent_kitten_teach_target"
        );
        assert_eq!(
            dependent_kitten_target_dse(Action::Release).id().0,
            "dependent_kitten_release_target"
        );
    }

    #[test]
    fn dse_has_one_axis() {
        for action in [Action::Wean, Action::Teach, Action::Release] {
            assert_eq!(
                dependent_kitten_target_dse(action)
                    .per_target_considerations()
                    .len(),
                1
            );
        }
    }

    #[test]
    fn dse_uses_best_aggregation() {
        for action in [Action::Wean, Action::Teach, Action::Release] {
            assert_eq!(
                dependent_kitten_target_dse(action).aggregation(),
                TargetAggregation::Best
            );
        }
    }

    #[test]
    #[should_panic(expected = "unsupported action")]
    fn dse_panics_on_unsupported_action() {
        let _ = dependent_kitten_target_dse(Action::Eat);
    }

    // -- Maturity band --------------------------------------------------------

    #[test]
    fn wean_band_is_below_weaned_threshold() {
        assert!(maturity_in_band(Action::Wean, 0.0, 0.33, 0.66));
        assert!(maturity_in_band(Action::Wean, 0.32, 0.33, 0.66));
        assert!(!maturity_in_band(Action::Wean, 0.33, 0.33, 0.66));
        assert!(!maturity_in_band(Action::Wean, 0.5, 0.33, 0.66));
    }

    #[test]
    fn teach_band_is_between_thresholds() {
        assert!(!maturity_in_band(Action::Teach, 0.32, 0.33, 0.66));
        assert!(maturity_in_band(Action::Teach, 0.33, 0.33, 0.66));
        assert!(maturity_in_band(Action::Teach, 0.5, 0.33, 0.66));
        assert!(!maturity_in_band(Action::Teach, 0.66, 0.33, 0.66));
    }

    #[test]
    fn release_band_is_at_or_above_teach_done() {
        assert!(!maturity_in_band(Action::Release, 0.5, 0.33, 0.66));
        assert!(maturity_in_band(Action::Release, 0.66, 0.33, 0.66));
        assert!(maturity_in_band(Action::Release, 1.0, 0.33, 0.66));
    }

    // -- Resolver boundary ----------------------------------------------------

    #[test]
    fn resolver_returns_none_when_dse_not_registered() {
        let registry = DseRegistry::new();
        let queen = Entity::from_raw_u32(1).unwrap();
        let out = resolve_dependent_kitten_target(
            Action::Wean,
            &registry,
            queen,
            Position::new(0, 0),
            &[],
            0.33,
            0.66,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_skips_kittens_with_other_mother() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Wean));
        let queen = Entity::from_raw_u32(1).unwrap();
        let other_mother = Entity::from_raw_u32(99).unwrap();
        let kittens = vec![kitten(10, 1, 0, 0.1, Some(other_mother))];
        let out = resolve_dependent_kitten_target(
            Action::Wean,
            &registry,
            queen,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0,
            None,
        );
        assert!(out.is_none(), "non-mother kittens should be rejected");
    }

    #[test]
    fn resolver_skips_kittens_outside_maturity_band() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Wean));
        let queen = Entity::from_raw_u32(1).unwrap();
        // Wean band: maturity < 0.33. This kitten is past the band.
        let kittens = vec![kitten(10, 1, 0, 0.5, Some(queen))];
        let out = resolve_dependent_kitten_target(
            Action::Wean,
            &registry,
            queen,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0,
            None,
        );
        assert!(out.is_none(), "out-of-band maturity should be rejected");
    }

    #[test]
    fn resolver_picks_in_band_kitten_when_only_one() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Teach));
        let queen = Entity::from_raw_u32(1).unwrap();
        let target_kitten = Entity::from_raw_u32(10).unwrap();
        let kittens = vec![
            kitten(10, 1, 0, 0.5, Some(queen)), // Teach band
            kitten(11, 2, 0, 0.1, Some(queen)), // Wean band — excluded
            kitten(12, 3, 0, 0.8, Some(queen)), // Release band — excluded
        ];
        let out = resolve_dependent_kitten_target(
            Action::Teach,
            &registry,
            queen,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0,
            None,
        );
        assert_eq!(out, Some(target_kitten));
    }

    #[test]
    fn resolver_picks_nearest_when_two_in_band() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Wean));
        let queen = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![
            kitten(10, 5, 0, 0.1, Some(queen)),
            kitten(11, 1, 0, 0.1, Some(queen)),
        ];
        let out = resolve_dependent_kitten_target(
            Action::Wean,
            &registry,
            queen,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0,
            None,
        );
        assert_eq!(out, Some(Entity::from_raw_u32(11).unwrap()));
    }

    #[test]
    fn resolver_drops_kittens_outside_range() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Release));
        let queen = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![kitten(10, 50, 0, 0.8, Some(queen))]; // dist >> 12
        let out = resolve_dependent_kitten_target(
            Action::Release,
            &registry,
            queen,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0,
            None,
        );
        assert!(out.is_none());
    }
}
