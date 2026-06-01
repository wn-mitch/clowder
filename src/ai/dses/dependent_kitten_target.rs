//! Ticket 364 — `DependentKittenTargetDse`. Pairs with the `rear_kitten`
//! HTN method's three primitive leaves (Wean / Teach / Release) registered
//! in `src/ai/methods/rear_kitten.rs`. The HTN dispatch closure pins
//! `chosen_action` to the leaf primitive at the L2 author site; this DSE
//! resolves the *target kitten* the leaf acts on.
//!
//! One factory parameterized by [`Action`] produces three sibling
//! `TargetTakingDse`s — same scoring shape, different DseId + eligibility
//! band (ticket 395 moved Release from `teach_done_threshold` to a
//! distinct `release_threshold`, leaving a deliberate gap where the arc
//! is idle and Caretake covers feeding):
//!
//! | Action     | Eligibility band                                       |
//! |------------|--------------------------------------------------------|
//! | `Wean`     | `maturity < weaned_threshold`                          |
//! | `Teach`    | `weaned_threshold <= maturity < teach_done_threshold`  |
//! | `Release`  | `maturity >= release_threshold` AND not yet released   |
//!
//! 395 widened the parent filter from `KittenDependency.mother == Some(self)`
//! to `mother == Some(self) OR father == Some(self)` — both parents pitch
//! in. The leaves are mutually exclusive on maturity, so at most one DSE
//! fires per kitten per tick.
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
    /// 395: father parental link, so the picker can resolve either
    /// parent (symmetric — both pitch in).
    pub father: Option<Entity>,
    /// 395: `RearKittenReleased` marker present on the kitten. The
    /// picker excludes already-released kittens so the second parent's
    /// concurrent frame can't re-witness `Feature::KittenReleased`.
    pub released_by_arc: bool,
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
    "kittens with KittenDependency.(mother | father) == Some(self) within DEPENDENT_KITTEN_TARGET_RANGE, matching the action's maturity band, and not yet released by the arc"
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
        state: GoalState::predicate("wean_kitten", |_, _| false),
        strategy: CommitmentStrategy::SingleMinded,
    }
}

fn teach_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState::predicate("teach_kitten", |_, _| false),
        strategy: CommitmentStrategy::SingleMinded,
    }
}

fn release_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState::predicate("release_kitten", |_, _| false),
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best dependent-kitten target for `parent` given the leaf
/// primitive's `action`. Returns `None` iff no kitten satisfies the
/// action's maturity band AND the mother-OR-father filter AND the
/// range gate AND the not-yet-released filter.
///
/// `kittens` is built upstream by the dispatch arm from
/// `ExecutorContext::kitten_parentage` joined against the cat-position
/// snapshot. Kittens without a recorded position are excluded by the
/// caller (cannot score nearness).
///
/// **395:** parameter renamed `queen` → `parent` (both parents
/// eligible), and a new `release_threshold` parameter gates the
/// Release band's lower bound. Picker also filters
/// `!released_by_arc` so the second parent's concurrent frame can't
/// re-witness Release.
#[allow(clippy::too_many_arguments)]
pub fn resolve_dependent_kitten_target(
    action: Action,
    registry: &DseRegistry,
    parent: Entity,
    parent_pos: Position,
    kittens: &[DependentKittenState],
    weaned_threshold: f32,
    teach_done_threshold: f32,
    release_threshold: f32,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
    // Ticket 427 Step 1 — pre-allocated scratch buffers.
    scratch: &mut crate::resources::DseTargetScratchpad,
) -> Option<Entity> {
    let dse_id = dependent_kitten_target_dse_id_for(action);
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == dse_id.0)?;

    scratch.entities.clear();
    scratch.positions.clear();
    for kitten in kittens {
        // 395: symmetric — either parent is eligible.
        if kitten.mother != Some(parent) && kitten.father != Some(parent) {
            continue;
        }
        // 395: arc fires Release exactly once per kitten via the
        // one-shot RearKittenReleased marker; second parent's frame
        // sees this and falls through to None → R11 Advance.
        if kitten.released_by_arc {
            continue;
        }
        if !maturity_in_band(
            action,
            kitten.maturity,
            weaned_threshold,
            teach_done_threshold,
            release_threshold,
        ) {
            continue;
        }
        let dist = parent_pos.distance_to(&kitten.pos);
        if dist > DEPENDENT_KITTEN_TARGET_RANGE {
            continue;
        }
        scratch.entities.push(kitten.entity);
        scratch.positions.push(kitten.pos);
    }

    if scratch.entities.is_empty() {
        return None;
    }

    let fetch_self = |_name: &str, _queen: Entity| -> f32 { 0.0 };
    let fetch_target = |_name: &str, _queen: Entity, _target: Entity| -> f32 { 0.0 };

    let entity_position = |_: Entity| -> Option<Position> { None };
    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat: parent,
        tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
        has_marker: &has_marker,
        self_position: parent_pos,
        target: None,
        target_position: None,
        target_alive: None,
        field_cost: None,
    };

    let scored = evaluate_target_taking(
        dse,
        parent,
        &scratch.entities,
        &scratch.positions,
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
            hook.capture.set_target_ranking(dse_id.0, ranking, tick);
        }
    }

    scored.winning_target
}

/// Per-action maturity-band predicate. 395: Release lower bound
/// switched from `teach_done_threshold` to `release_threshold` (the
/// near-mature window). The gap `[teach_done_threshold,
/// release_threshold)` is deliberate idle space where the arc emits
/// nothing and Caretake covers feeding.
pub fn maturity_in_band(
    action: Action,
    maturity: f32,
    weaned_threshold: f32,
    teach_done_threshold: f32,
    release_threshold: f32,
) -> bool {
    match action {
        Action::Wean => maturity < weaned_threshold,
        Action::Teach => maturity >= weaned_threshold && maturity < teach_done_threshold,
        Action::Release => maturity >= release_threshold,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kitten(
        id: u32,
        x: i32,
        y: i32,
        maturity: f32,
        mother: Option<Entity>,
    ) -> DependentKittenState {
        DependentKittenState {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Position::new(x, y),
            maturity,
            mother,
            father: None,
            released_by_arc: false,
        }
    }

    fn kitten_with_father(
        id: u32,
        x: i32,
        y: i32,
        maturity: f32,
        mother: Option<Entity>,
        father: Option<Entity>,
    ) -> DependentKittenState {
        DependentKittenState {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Position::new(x, y),
            maturity,
            mother,
            father,
            released_by_arc: false,
        }
    }

    fn kitten_released(
        id: u32,
        x: i32,
        y: i32,
        maturity: f32,
        mother: Option<Entity>,
    ) -> DependentKittenState {
        DependentKittenState {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Position::new(x, y),
            maturity,
            mother,
            father: None,
            released_by_arc: true,
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
        assert!(maturity_in_band(Action::Wean, 0.0, 0.33, 0.66, 0.95));
        assert!(maturity_in_band(Action::Wean, 0.32, 0.33, 0.66, 0.95));
        assert!(!maturity_in_band(Action::Wean, 0.33, 0.33, 0.66, 0.95));
        assert!(!maturity_in_band(Action::Wean, 0.5, 0.33, 0.66, 0.95));
    }

    #[test]
    fn teach_band_is_between_thresholds() {
        assert!(!maturity_in_band(Action::Teach, 0.32, 0.33, 0.66, 0.95));
        assert!(maturity_in_band(Action::Teach, 0.33, 0.33, 0.66, 0.95));
        assert!(maturity_in_band(Action::Teach, 0.5, 0.33, 0.66, 0.95));
        assert!(!maturity_in_band(Action::Teach, 0.66, 0.33, 0.66, 0.95));
    }

    #[test]
    fn release_band_is_at_or_above_release_threshold() {
        // 395: Release band lower bound is release_threshold (0.95),
        // not teach_done_threshold (0.66). The gap [0.66, 0.95) is
        // deliberate idle space (Caretake covers feeding there).
        assert!(!maturity_in_band(Action::Release, 0.5, 0.33, 0.66, 0.95));
        assert!(!maturity_in_band(Action::Release, 0.66, 0.33, 0.66, 0.95));
        assert!(!maturity_in_band(Action::Release, 0.9, 0.33, 0.66, 0.95));
        assert!(maturity_in_band(Action::Release, 0.95, 0.33, 0.66, 0.95));
        assert!(maturity_in_band(Action::Release, 1.0, 0.33, 0.66, 0.95));
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
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_skips_kittens_unrelated_to_parent() {
        // 395: with symmetric matching, kittens whose neither parent
        // matches `parent` are rejected.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Wean));
        let parent = Entity::from_raw_u32(1).unwrap();
        let other_mother = Entity::from_raw_u32(99).unwrap();
        let other_father = Entity::from_raw_u32(98).unwrap();
        let kittens = vec![kitten_with_father(
            10,
            1,
            0,
            0.1,
            Some(other_mother),
            Some(other_father),
        )];
        let out = resolve_dependent_kitten_target(
            Action::Wean,
            &registry,
            parent,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none(), "unrelated kittens should be rejected");
    }

    #[test]
    fn resolver_matches_via_father_too() {
        // 395 symmetric: father can fire the arc when picker filter
        // sees `father == Some(parent)`.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Wean));
        let father = Entity::from_raw_u32(1).unwrap();
        let mother = Entity::from_raw_u32(99).unwrap();
        let target_kitten = Entity::from_raw_u32(10).unwrap();
        let kittens = vec![kitten_with_father(
            10,
            1,
            0,
            0.1,
            Some(mother),
            Some(father),
        )];
        let out = resolve_dependent_kitten_target(
            Action::Wean,
            &registry,
            father,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(
            out,
            Some(target_kitten),
            "father should match via symmetric picker"
        );
    }

    #[test]
    fn resolver_skips_released_kittens() {
        // 395 one-shot: a kitten with RearKittenReleased marker (set
        // by the first parent's Release drain) is excluded so the
        // second parent's frame can't re-witness.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Release));
        let mother = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![kitten_released(10, 1, 0, 0.97, Some(mother))];
        let out = resolve_dependent_kitten_target(
            Action::Release,
            &registry,
            mother,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none(), "released kittens should be rejected");
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
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none(), "out-of-band maturity should be rejected");
    }

    #[test]
    fn resolver_skips_kittens_in_idle_gap() {
        // 395: maturity 0.8 sits in [teach_done=0.66, release_thresh=0.95)
        // — deliberate idle gap. Release picker rejects.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(dependent_kitten_target_dse(Action::Release));
        let mother = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![kitten(10, 1, 0, 0.8, Some(mother))];
        let out = resolve_dependent_kitten_target(
            Action::Release,
            &registry,
            mother,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none(), "idle-gap maturity should be rejected");
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
            kitten(10, 1, 0, 0.5, Some(queen)),  // Teach band
            kitten(11, 2, 0, 0.1, Some(queen)),  // Wean band — excluded
            kitten(12, 3, 0, 0.97, Some(queen)), // Release band — excluded
        ];
        let out = resolve_dependent_kitten_target(
            Action::Teach,
            &registry,
            queen,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
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
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
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
        // 395: kitten at maturity 0.97 (Release band) but far away.
        let kittens = vec![kitten(10, 50, 0, 0.97, Some(queen))]; // dist >> 12
        let out = resolve_dependent_kitten_target(
            Action::Release,
            &registry,
            queen,
            Position::new(0, 0),
            &kittens,
            0.33,
            0.66,
            0.95,
            0,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }
}
