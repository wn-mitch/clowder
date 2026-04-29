//! `CaretakeTargetDse` — §6.5.6 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning kitten selection for `Caretake`. Pairs with
//! the self-state [`CaretakeDse`](super::caretake::CaretakeDse) which
//! decides *whether* to caretake (via kitten_urgency × compassion ×
//! is_parent); this DSE decides *whom*.
//!
//! Phase 4c.7 scope: retire the plain-helper `resolve_caretake` in
//! `src/ai/caretake_targeting.rs` and cut
//! `disposition.rs::build_caretaking_chain` +
//! `goap.rs::FeedKitten` populate-sites over to the spec-shape
//! four-axis bundle. The §6.5.6 axes are:
//!
//! | # | Consideration       | Source                     | Curve                                   | Weight |
//! |---|---------------------|----------------------------|-----------------------------------------|--------|
//! | 1 | distance            | `Spatial(target)`          | `Quadratic(exp=1.5, div=-1, shift=1)`   | 0.20   |
//! | 2 | kitten-hunger       | `target_kitten_hunger`     | `Quadratic(exp=2)`                      | 0.40   |
//! | 3 | kinship             | `target_kinship`           | `Piecewise([(0, 0.6), (1, 1)])`         | 0.25   |
//! | 4 | kitten-isolation    | `target_kitten_isolation`  | `Linear(1, 0)`                          | 0.15   |
//!
//! Weights sum to 1.0 verbatim — no deferred axes. The distance axis
//! lands as a `SpatialConsideration` per the §L2.10.7 plan-cost
//! feedback design (ticket 052) — `Quadratic(exp=1.5, divisor=-1,
//! shift=1)` over normalized cost evaluates `(1 - cost)^1.5`,
//! preserving the legacy `nearness^1.5` shape (same explicit-
//! inversion idiom as ApplyRemedy).
//!
//! **Aggregation.** `Best` — §6.6 default. Pre-refactor
//! `resolve_caretake` picked argmax of a hand-rolled
//! `deficit × decay × kinship_boost` product; the new bundle
//! preserves the argmax shape while making kitten-isolation and the
//! Quadratic convex amplification on hunger visible as separate axes.
//!
//! **Composition.** `WeightedSum` matches the social-family pattern
//! (Socialize / Mate / Mentor / GroomOther). `CompensatedProduct` would
//! gate any low axis — a near-distance-but-moderately-hungry kitten
//! with no parent in the colony would score 0 (kinship axis ≈ 0.6 via
//! the Piecewise floor, but isolation axis = 0 when siblings are
//! adjacent), which over-punishes the common case. The hunger axis's
//! weight (0.40) is dominant by design: hunger is the defining Caretake
//! signal per §6.5.6.
//!
//! **Kinship curve.** The spec names this a `Cliff(parent=1.0, non-parent=0.6)`.
//! `Curve::Piecewise` with knots `[(0.0, 0.6), (1.0, 1.0)]` yields
//! exactly that on binary input `{0.0, 1.0}` — non-parents hit the
//! 0.6 floor, parents hit 1.0. Baseline 0.6 (not 0.0) keeps non-parent
//! adults responsive to hungry kittens, preserving the colony-raising
//! pattern Phase 4c.4's alloparenting Reframe A established.
//!
//! **Isolation curve.** Linear(1, 0) on a binary "no sibling or parent
//! within 3 tiles" signal: `1.0` when isolated, `0.0` when a sibling or
//! parent is co-located. Protects the edge case that motivates
//! per-kitten targeting over per-store in the first place — a wandered
//! orphan in the woods must outscore a well-attended kitten beside the
//! hearth.
//!
//! ## What this DSE does *not* own
//!
//! - `is_parent_of_hungry_kitten` — the self-state `CaretakeDse`
//!   scoring axis that bumps parents even when they're far from the
//!   kitten (bloodline-override signal). Caller-side responsibility:
//!   [`resolve_caretake_target`] walks the kitten snapshot and reports
//!   `is_parent` in the [`CaretakeResolution`] return value.
//! - `caretake_compassion` — the bond-weighted compassion scalar
//!   (Phase 4c.4 alloparenting Reframe A). That scaling stays in
//!   [`caretake_compassion_bond_scale`](crate::ai::caretake_targeting::caretake_compassion_bond_scale)
//!   unchanged — the `target_mother` / `target_father` fields on the
//!   returned resolution feed it directly.
//! - `hungry_kitten_urgency` scoring axis on `CaretakeDse` — fed by
//!   the aggregated score (§6.6 `Best` → max per-candidate score)
//!   returned in [`CaretakeResolution::urgency`].

use bevy::prelude::Entity;

use crate::ai::caretake_targeting::{CaretakeResolution, KittenState};
use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkSource, ScalarConsideration, SpatialConsideration, LandmarkAnchor};
use crate::ai::curves::Curve;
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;

pub const TARGET_KITTEN_HUNGER_INPUT: &str = "target_kitten_hunger";
pub const TARGET_KINSHIP_INPUT: &str = "target_kinship";
pub const TARGET_KITTEN_ISOLATION_INPUT: &str = "target_kitten_isolation";

/// Candidate-pool range in Manhattan tiles. Matches spec §6.4 row #9
/// (range=12) and the pre-refactor `CARETAKE_RANGE` constant —
/// parents cross the colony for a hungry kitten.
pub const CARETAKE_TARGET_RANGE: f32 = 12.0;

/// Kittens below this hunger threshold are candidates for Caretake.
/// Hunger is satisfaction (1.0 = sated, 0.0 = starving); kittens under
/// 0.6 triage in. Preserved verbatim from pre-refactor to keep the
/// eligibility population stable across the port — the Quadratic(2)
/// hunger axis does the heavy lifting inside the gate.
pub const KITTEN_HUNGER_THRESHOLD: f32 = 0.6;

/// "Isolated" means no sibling or parent is within this many Manhattan
/// tiles. Three tiles is tight enough to mean "in the same scene" per
/// §6.5.6's rationale — a wandered-off kitten or an orphan.
pub const ISOLATION_RADIUS: f32 = 3.0;

/// §6.5.6 `Caretake` target-taking DSE factory.
pub fn caretake_target_dse() -> TargetTakingDse {
    // §L2.10.7 distance axis: `(1 - cost)^1.5` via `Quadratic(exp=1.5,
    // divisor=-1, shift=1)`. Same explicit-inversion idiom as
    // ApplyRemedy / Mentor / Socialize ports.
    let nearness_curve = Curve::Quadratic {
        exponent: 1.5,
        divisor: -1.0,
        shift: 1.0,
    };
    let hunger_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    // Kinship Cliff: parent=1.0, non-parent=0.6. Piecewise with two
    // knots handles the binary signal exactly — interpolation between
    // 0.0 and 1.0 never fires because the signal is always one or the
    // other, but the floor (0.6) keeps non-parents in play.
    let kinship_curve = Curve::Piecewise {
        knots: vec![(0.0, 0.6), (1.0, 1.0)],
    };
    let isolation_curve = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };

    TargetTakingDse {
        id: DseId("caretake_target"),
        candidate_query: caretake_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Spatial(SpatialConsideration::new(
                "caretake_target_nearness",
                LandmarkSource::TargetPosition,
                CARETAKE_TARGET_RANGE,
                nearness_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_KITTEN_HUNGER_INPUT,
                hunger_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_KINSHIP_INPUT,
                kinship_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_KITTEN_ISOLATION_INPUT,
                isolation_curve,
            )),
        ],
        composition: Composition::weighted_sum(vec![0.20, 0.40, 0.25, 0.15]),
        aggregation: TargetAggregation::Best,
        intention: caretake_intention,
        required_stance: None,
        // Ticket 080 — caretake (kitten feeding) is contention-tolerant.
        eligibility: Default::default(),
    }
}

fn caretake_candidate_query_doc(_cat: Entity) -> &'static str {
    "kittens with hunger < KITTEN_HUNGER_THRESHOLD within CARETAKE_TARGET_RANGE"
}

fn caretake_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "kitten_fed",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best kitten for `adult` via the registered
/// [`caretake_target_dse`]. Returns a [`CaretakeResolution`] whose
/// `urgency` carries the aggregated-`Best` score (the winning
/// kitten's per-target composite), `target` / `target_pos` carry the
/// argmax kitten's entity + position, and `target_mother` /
/// `target_father` carry its `KittenDependency` parents for the
/// compassion-bond scaling in
/// [`caretake_compassion_bond_scale`](crate::ai::caretake_targeting::caretake_compassion_bond_scale).
///
/// `is_parent` is `true` iff **any** hungry kitten in range is the
/// adult's own offspring — matches the pre-refactor semantics that
/// feeds the `CaretakeDse::is_parent_of_hungry_kitten` scoring axis
/// (bloodline-override works even if the winning argmax kitten is not
/// the adult's own, e.g. when a colony-kitten is nearer).
///
/// Returns `CaretakeResolution::default()` (all-None / 0.0) when no
/// hungry kitten is in range or the DSE isn't registered — matches the
/// pre-refactor contract.
pub fn resolve_caretake_target(
    registry: &DseRegistry,
    adult: Entity,
    adult_pos: Position,
    kittens: &[KittenState],
    cat_positions: &[(Entity, Position)],
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> CaretakeResolution {
    let Some(dse) = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "caretake_target")
    else {
        return CaretakeResolution::default();
    };

    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    let mut any_parent_hit = false;

    for kitten in kittens {
        if kitten.hunger >= KITTEN_HUNGER_THRESHOLD {
            continue;
        }
        let dist = adult_pos.manhattan_distance(&kitten.pos) as f32;
        if dist > CARETAKE_TARGET_RANGE {
            continue;
        }
        if kitten.mother == Some(adult) || kitten.father == Some(adult) {
            any_parent_hit = true;
        }
        candidates.push(kitten.entity);
        positions.push(kitten.pos);
    }

    if candidates.is_empty() {
        return CaretakeResolution::default();
    }

    // Per-target lookup tables — keyed on the candidate entity so the
    // target-fetcher closure can resolve without re-scanning.
    let mut kitten_by_entity: std::collections::HashMap<Entity, KittenState> =
        std::collections::HashMap::with_capacity(candidates.len());
    for k in kittens {
        kitten_by_entity.insert(k.entity, *k);
    }

    // Spatial nearness axis (`caretake_target_nearness`) is computed
    // by the substrate from `EvalCtx::self_position` to each
    // candidate's tile per §L2.10.7.
    let fetch_self = |_name: &str, _adult: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, _adult: Entity, target: Entity| -> f32 {
        match name {
            TARGET_KITTEN_HUNGER_INPUT => kitten_by_entity
                .get(&target)
                .map(|k| (1.0 - k.hunger).clamp(0.0, 1.0))
                .unwrap_or(0.0),
            TARGET_KINSHIP_INPUT => {
                let k = match kitten_by_entity.get(&target) {
                    Some(k) => k,
                    None => return 0.0,
                };
                if k.mother == Some(adult) || k.father == Some(adult) {
                    1.0
                } else {
                    0.0
                }
            }
            TARGET_KITTEN_ISOLATION_INPUT => {
                let k = match kitten_by_entity.get(&target) {
                    Some(k) => k,
                    None => return 0.0,
                };
                if is_kitten_isolated(k, kittens, cat_positions) {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    };

    let entity_position = |_: Entity| -> Option<Position> { None };

    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat: adult,
        tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
        has_marker: &has_marker,
        self_position: adult_pos,
        target: None,
        target_position: None,
    };

    let scored = evaluate_target_taking(
        dse,
        adult,
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
                .set_target_ranking("caretake_target", ranking, tick);
        }
    }

    let Some(winner) = scored.winning_target else {
        return CaretakeResolution::default();
    };
    let winning_kitten = match kitten_by_entity.get(&winner) {
        Some(k) => *k,
        None => return CaretakeResolution::default(),
    };

    CaretakeResolution {
        urgency: scored.aggregated_score.clamp(0.0, 1.0),
        is_parent: any_parent_hit,
        target: Some(winner),
        target_pos: Some(winning_kitten.pos),
        target_mother: winning_kitten.mother,
        target_father: winning_kitten.father,
    }
}

/// A kitten is isolated when no sibling (kitten sharing either parent)
/// and no parent (adult matching the kitten's `mother` or `father`)
/// sits within `ISOLATION_RADIUS` Manhattan tiles. Sated / well-fed
/// kittens still count as siblings for the purpose of co-location —
/// the isolation signal describes *who is nearby*, not *who else needs
/// caretaking*.
fn is_kitten_isolated(
    kitten: &KittenState,
    kittens: &[KittenState],
    cat_positions: &[(Entity, Position)],
) -> bool {
    for other in kittens {
        if other.entity == kitten.entity {
            continue;
        }
        let shares_mother = kitten.mother.zip(other.mother).is_some_and(|(a, b)| a == b);
        let shares_father = kitten.father.zip(other.father).is_some_and(|(a, b)| a == b);
        if (shares_mother || shares_father)
            && kitten.pos.manhattan_distance(&other.pos) as f32 <= ISOLATION_RADIUS
        {
            return false;
        }
    }
    for (entity, pos) in cat_positions {
        let is_parent = kitten.mother == Some(*entity) || kitten.father == Some(*entity);
        if is_parent && kitten.pos.manhattan_distance(pos) as f32 <= ISOLATION_RADIUS {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kitten(id: u32, x: i32, y: i32, hunger: f32) -> KittenState {
        KittenState {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Position::new(x, y),
            hunger,
            mother: None,
            father: None,
        }
    }

    fn kitten_with_parents(
        id: u32,
        x: i32,
        y: i32,
        hunger: f32,
        mother: Option<Entity>,
        father: Option<Entity>,
    ) -> KittenState {
        KittenState {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Position::new(x, y),
            hunger,
            mother,
            father,
        }
    }

    // -- Factory shape --------------------------------------------------------

    #[test]
    fn caretake_target_dse_id_stable() {
        assert_eq!(caretake_target_dse().id().0, "caretake_target");
    }

    #[test]
    fn caretake_target_has_four_axes() {
        assert_eq!(caretake_target_dse().per_target_considerations().len(), 4);
    }

    #[test]
    fn caretake_target_weights_sum_to_one() {
        let sum: f32 = caretake_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "weights sum {sum} ≠ 1.0");
    }

    #[test]
    fn caretake_target_uses_best_aggregation() {
        assert_eq!(caretake_target_dse().aggregation(), TargetAggregation::Best);
    }

    #[test]
    fn intention_is_kitten_fed_goal() {
        let dse = caretake_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label, "kitten_fed");
                assert_eq!(strategy, CommitmentStrategy::SingleMinded);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    // -- Resolver boundary behavior ------------------------------------------

    #[test]
    fn resolver_returns_default_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let adult = Entity::from_raw_u32(1).unwrap();
        let out = resolve_caretake_target(&registry, adult, Position::new(0, 0), &[], &[], 0, None);
        assert!(out.target.is_none());
        assert_eq!(out.urgency, 0.0);
        assert!(!out.is_parent);
    }

    #[test]
    fn resolver_returns_default_with_empty_kittens() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let out = resolve_caretake_target(&registry, adult, Position::new(0, 0), &[], &[], 0, None);
        assert!(out.target.is_none());
    }

    #[test]
    fn resolver_skips_well_fed_kittens() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![kitten(10, 1, 0, 0.9), kitten(11, 2, 0, 0.8)];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert!(out.target.is_none());
    }

    #[test]
    fn resolver_filters_out_of_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        // Hungry but beyond CARETAKE_TARGET_RANGE (12).
        let kittens = vec![kitten(10, 50, 0, 0.1)];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert!(out.target.is_none());
    }

    // -- Axis semantics ------------------------------------------------------

    #[test]
    fn picks_hungrier_kitten_when_distance_tied() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        // Both at distance 2; the Quadratic(2) hunger axis amplifies
        // the one with the bigger deficit.
        let kittens = vec![
            kitten(10, 2, 0, 0.4), // deficit 0.6 → hunger axis 0.36
            kitten(11, 0, 2, 0.1), // deficit 0.9 → hunger axis 0.81
        ];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert_eq!(out.target, Some(Entity::from_raw_u32(11).unwrap()));
    }

    #[test]
    fn kinship_piecewise_floor_still_picks_non_parent_when_only_candidate() {
        // Single non-parent candidate — kinship axis drops to 0.6
        // via the Piecewise floor rather than 0. Adult still picks
        // this kitten (colony-raising pattern).
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let other = Entity::from_raw_u32(99).unwrap();
        let kittens = vec![kitten_with_parents(10, 1, 0, 0.2, Some(other), None)];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert_eq!(out.target, Some(Entity::from_raw_u32(10).unwrap()));
        assert!(out.urgency > 0.0);
        assert!(!out.is_parent, "adult is not mother/father of candidate");
    }

    #[test]
    fn own_kitten_beats_similar_stranger_via_kinship_cliff() {
        // Two kittens at matched distance + hunger; only kinship
        // distinguishes them. Own-kitten's kinship=1.0 beats
        // stranger's 0.6.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![
            kitten_with_parents(10, 2, 0, 0.2, None, None), // stranger
            kitten_with_parents(11, 0, 2, 0.2, Some(adult), None), // own kitten
        ];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert_eq!(out.target, Some(Entity::from_raw_u32(11).unwrap()));
        assert!(out.is_parent, "own-kitten hit sets is_parent");
    }

    #[test]
    fn is_parent_fires_even_when_stranger_wins_argmax() {
        // Stranger is closer + much hungrier → stranger wins argmax.
        // But adult's own kitten is in range → `is_parent` still true,
        // so CaretakeDse's bloodline-override axis fires.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![
            kitten_with_parents(10, 1, 0, 0.05, None, None), // stranger
            kitten_with_parents(11, 10, 0, 0.5, Some(adult), None), // own kitten, far, less hungry
        ];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert_eq!(out.target, Some(Entity::from_raw_u32(10).unwrap()));
        assert!(
            out.is_parent,
            "own kitten in range → is_parent must stay true"
        );
    }

    #[test]
    fn closer_kitten_wins_when_hunger_equal() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let kittens = vec![
            kitten(10, 1, 0, 0.2), // dist 1
            kitten(11, 5, 0, 0.2), // dist 5
        ];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert_eq!(out.target, Some(Entity::from_raw_u32(10).unwrap()));
    }

    // -- Isolation axis -------------------------------------------------------

    #[test]
    fn isolated_kitten_beats_co_located_sibling_all_else_equal() {
        // Two kittens tied on distance + hunger + kinship (both
        // strangers to the adult). One has a sibling beside it; the
        // other is alone. Isolation axis (weight 0.15) breaks the tie.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let shared_mother = Entity::from_raw_u32(99).unwrap();
        let kittens = vec![
            // Lonely: has a mother, but no sibling or parent within 3.
            kitten_with_parents(10, 3, 0, 0.2, Some(shared_mother), None),
            // Co-located pair: kittens 11 + 12 share a mother and sit adjacent.
            kitten_with_parents(11, 0, 3, 0.2, Some(shared_mother), None),
            kitten_with_parents(12, 1, 3, 0.5, Some(shared_mother), None),
        ];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert_eq!(
            out.target,
            Some(Entity::from_raw_u32(10).unwrap()),
            "lonely kitten should outscore co-located sibling at tied distance/hunger"
        );
    }

    #[test]
    fn parent_presence_suppresses_isolation() {
        // Kitten has mother (entity 99) standing beside it → not
        // isolated. Second kitten at equal distance + hunger but no
        // parent adjacent → isolated. Axis favors the orphan-like one.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let mother = Entity::from_raw_u32(99).unwrap();
        let kittens = vec![
            // Attended: mother at (4, 0), kitten at (3, 0) → parent within 3.
            kitten_with_parents(10, 3, 0, 0.2, Some(mother), None),
            // Orphan: no parent in the scene.
            kitten_with_parents(11, 0, 3, 0.2, None, None),
        ];
        let cat_positions = vec![(mother, Position::new(4, 0))];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &cat_positions,
            0,
            None,
        );
        assert_eq!(
            out.target,
            Some(Entity::from_raw_u32(11).unwrap()),
            "orphan (isolated) should outscore kitten with mother adjacent"
        );
    }

    // -- Resolution surface parity -------------------------------------------

    #[test]
    fn resolution_surfaces_target_parents() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        let mother = Entity::from_raw_u32(20).unwrap();
        let father = Entity::from_raw_u32(30).unwrap();
        let kittens = vec![kitten_with_parents(
            10,
            1,
            0,
            0.2,
            Some(mother),
            Some(father),
        )];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert_eq!(out.target_mother, Some(mother));
        assert_eq!(out.target_father, Some(father));
        assert_eq!(out.target_pos, Some(Position::new(1, 0)));
    }

    #[test]
    fn urgency_is_in_unit_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(caretake_target_dse());
        let adult = Entity::from_raw_u32(1).unwrap();
        // Worst-case: near-starving own kitten, adjacent, isolated.
        let kittens = vec![kitten_with_parents(10, 1, 0, 0.0, Some(adult), None)];
        let out = resolve_caretake_target(
            &registry,
            adult,
            Position::new(0, 0),
            &kittens,
            &[],
            0,
            None,
        );
        assert!(out.target.is_some());
        assert!(
            (0.0..=1.0).contains(&out.urgency),
            "urgency {} out of [0, 1]",
            out.urgency
        );
        assert!(
            out.urgency > 0.5,
            "urgent own-kitten-starving case should score > 0.5, got {}",
            out.urgency
        );
    }
}
