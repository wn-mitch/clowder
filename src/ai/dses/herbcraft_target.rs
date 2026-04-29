//! `HerbcraftTargetDse` — §6.5 target-taking DSE owning herb-patch
//! selection for `Herbcraft::Gather`. Pairs with the self-state
//! [`HerbcraftGatherDse`](super::herbcraft_gather::HerbcraftGatherDse)
//! which decides *whether* to gather; this DSE decides *which herb*.
//!
//! Ticket 061 lands the producer-side `HerbLocationMap` (§5.6.3 row #8)
//! and authors this minimum-viable consumer so the map has a reader on
//! day one. Per-DSE numeric balance tuning (kind-need matching, etc.)
//! is out of scope and lives in ticket 052 + balance threads.
//!
//! Three per-target considerations, `WeightedSum`, aggregation `Best`:
//!
//! | # | Consideration       | Source                       | Curve                           | Weight |
//! |---|---------------------|------------------------------|---------------------------------|--------|
//! | 1 | distance            | `Spatial(target)`            | `Linear(slope=-1, intercept=1)` | 0.40   |
//! | 2 | patch-density       | `target_herb_density`        | `Linear(1, 0)`                  | 0.40   |
//! | 3 | maturity            | `target_herb_maturity`       | `Linear(1, 0)`                  | 0.20   |
//!
//! **Distance** uses the same `1 - cost` shape as `BuildTargetDse`
//! (substrate normalizes to `cost = dist / range`).
//!
//! **Patch-density** samples `HerbLocationMap::total(target_pos)` —
//! the sum of all per-kind densities clamped to 1.0 within the
//! candidate's bucket. A herb in a dense thornbriar patch outscores a
//! lone sprout in the woods. Reads only the substrate; the map is
//! authored each tick by `update_herb_location_map`.
//!
//! **Maturity** is the per-target growth-stage strength
//! (`Sprout=0.25 → Blossom=1.0`). Distinct from density — density is
//! "is this tile in a patch?", maturity is "is this individual ripe?".
//! A blossoming herb at the edge of a patch beats a sprouting herb at
//! its center on this axis.
//!
//! ## Output contract
//!
//! Emits `Intention::Goal { state: "herbs_in_inventory" }` matching
//! [`HerbcraftGatherDse`] verbatim — the planner's existing
//! `gather_herb` step sequence keys on the goal state, so the
//! target-taking variant slots into the existing dispatch without a
//! new step. Caller-side responsibility: feed
//! `ScoredTargetTakingDse::winning_target` into the GOAP plan as the
//! target entity.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::Curve;
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::magic::{GrowthStage, HerbKind};
use crate::components::physical::Position;
use crate::resources::{growth_stage_strength, HerbLocationMap};

pub const TARGET_HERB_DENSITY_INPUT: &str = "target_herb_density";
pub const TARGET_HERB_MATURITY_INPUT: &str = "target_herb_maturity";

/// Candidate-pool range in Manhattan tiles. Matches the legacy
/// [`HERBCRAFT_GATHER_PATCH_RANGE`](super::herbcraft_gather::HERBCRAFT_GATHER_PATCH_RANGE)
/// — herbs are routine errands at colony commute scale.
pub const HERBCRAFT_TARGET_RANGE: f32 = 20.0;

/// Per-herb snapshot fed to [`resolve_herbcraft_target`].
#[derive(Clone, Copy, Debug)]
pub struct HerbCandidate {
    pub entity: Entity,
    pub position: Position,
    pub kind: HerbKind,
    pub growth_stage: GrowthStage,
}

/// `Herbcraft::Gather` target-taking DSE factory.
pub fn herbcraft_target_dse() -> TargetTakingDse {
    let nearness_curve = Curve::Linear {
        slope: -1.0,
        intercept: 1.0,
    };
    let density_curve = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    let maturity_curve = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };

    TargetTakingDse {
        id: DseId("herbcraft_target"),
        candidate_query: herbcraft_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Spatial(SpatialConsideration::new(
                "herbcraft_target_nearness",
                LandmarkSource::TargetPosition,
                HERBCRAFT_TARGET_RANGE,
                nearness_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_HERB_DENSITY_INPUT,
                density_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_HERB_MATURITY_INPUT,
                maturity_curve,
            )),
        ],
        composition: Composition::weighted_sum(vec![0.40, 0.40, 0.20]),
        aggregation: TargetAggregation::Best,
        intention: herbcraft_intention,
        required_stance: None,
    }
}

fn herbcraft_candidate_query_doc(_cat: Entity) -> &'static str {
    "harvestable herbs within HERBCRAFT_TARGET_RANGE"
}

fn herbcraft_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "herbs_in_inventory",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best harvestable herb for `cat` via the registered
/// [`herbcraft_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
pub fn resolve_herbcraft_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    candidates: &[HerbCandidate],
    map: &HerbLocationMap,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "herbcraft_target")?;

    if candidates.is_empty() {
        return None;
    }

    let mut entities: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    let mut density_by_entity: std::collections::HashMap<Entity, f32> =
        std::collections::HashMap::new();
    let mut maturity_by_entity: std::collections::HashMap<Entity, f32> =
        std::collections::HashMap::new();
    for c in candidates {
        let dist = cat_pos.manhattan_distance(&c.position) as f32;
        if dist > HERBCRAFT_TARGET_RANGE {
            continue;
        }
        entities.push(c.entity);
        positions.push(c.position);
        density_by_entity.insert(c.entity, map.total(c.position.x, c.position.y));
        maturity_by_entity.insert(c.entity, growth_stage_strength(c.growth_stage));
    }

    if entities.is_empty() {
        return None;
    }

    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, _cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_HERB_DENSITY_INPUT => density_by_entity.get(&target).copied().unwrap_or(0.0),
            TARGET_HERB_MATURITY_INPUT => maturity_by_entity.get(&target).copied().unwrap_or(0.0),
            _ => 0.0,
        }
    };

    let entity_position = |_: Entity| -> Option<Position> { None };
    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat,
        tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
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

    if let Some(hook) = focal_hook {
        if let Some(ranking) = crate::ai::target_dse::target_ranking_from_scored(
            &scored,
            dse.aggregation(),
            hook.name_lookup,
        ) {
            hook.capture
                .set_target_ranking("herbcraft_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn herb(id: u32, x: i32, y: i32, kind: HerbKind, stage: GrowthStage) -> HerbCandidate {
        HerbCandidate {
            entity: Entity::from_raw_u32(id).unwrap(),
            position: Position::new(x, y),
            kind,
            growth_stage: stage,
        }
    }

    fn empty_map() -> HerbLocationMap {
        HerbLocationMap::default_map()
    }

    // -- Factory shape --------------------------------------------------------

    #[test]
    fn herbcraft_target_dse_id_stable() {
        assert_eq!(herbcraft_target_dse().id().0, "herbcraft_target");
    }

    #[test]
    fn herbcraft_target_has_three_axes() {
        assert_eq!(herbcraft_target_dse().per_target_considerations().len(), 3);
    }

    #[test]
    fn herbcraft_target_weights_sum_to_one() {
        let sum: f32 = herbcraft_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "weights sum {sum} ≠ 1.0");
    }

    #[test]
    fn herbcraft_target_uses_best_aggregation() {
        assert_eq!(
            herbcraft_target_dse().aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn intention_is_herbs_in_inventory_goal() {
        let dse = herbcraft_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label, "herbs_in_inventory");
                assert_eq!(strategy, CommitmentStrategy::SingleMinded);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    // -- Resolver boundary behavior ------------------------------------------

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let map = empty_map();
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_herbcraft_target(&registry, cat, Position::new(0, 0), &[], &map, 0, None);
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_with_empty_candidates() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(herbcraft_target_dse());
        let map = empty_map();
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_herbcraft_target(&registry, cat, Position::new(0, 0), &[], &map, 0, None);
        assert!(out.is_none());
    }

    #[test]
    fn resolver_filters_out_of_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(herbcraft_target_dse());
        let map = empty_map();
        let cat = Entity::from_raw_u32(1).unwrap();
        // Beyond HERBCRAFT_TARGET_RANGE (20) Manhattan tiles.
        let far = herb(2, 50, 0, HerbKind::HealingMoss, GrowthStage::Blossom);
        let out = resolve_herbcraft_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[far],
            &map,
            0,
            None,
        );
        assert!(out.is_none());
    }

    // -- Axis semantics ------------------------------------------------------

    #[test]
    fn closer_herb_wins_when_density_and_maturity_tied() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(herbcraft_target_dse());
        let mut map = empty_map();
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = herb(2, 2, 0, HerbKind::HealingMoss, GrowthStage::Bloom);
        let far = herb(3, 15, 0, HerbKind::HealingMoss, GrowthStage::Bloom);
        // Both stamped equally (single isolated herb each) — density
        // axis ties.
        map.stamp(close.kind, close.position.x, close.position.y, 0.75, 15.0);
        map.stamp(far.kind, far.position.x, far.position.y, 0.75, 15.0);
        let out = resolve_herbcraft_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[close, far],
            &map,
            0,
            None,
        );
        assert_eq!(out, Some(close.entity));
    }

    #[test]
    fn riper_herb_beats_sprout_at_equal_distance_and_density() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(herbcraft_target_dse());
        // Empty map → density axis is 0 for both, leaving distance + maturity
        // to decide.
        let map = empty_map();
        let cat = Entity::from_raw_u32(1).unwrap();
        // Both at distance 5 (Manhattan); maturity differs.
        let blossom = herb(2, 5, 0, HerbKind::Thornbriar, GrowthStage::Blossom);
        let sprout = herb(3, 0, 5, HerbKind::Thornbriar, GrowthStage::Sprout);
        let out = resolve_herbcraft_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[blossom, sprout],
            &map,
            0,
            None,
        );
        assert_eq!(out, Some(blossom.entity));
    }

    #[test]
    fn dense_patch_outscores_lone_herb_at_equal_distance_and_maturity() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(herbcraft_target_dse());
        let mut map = empty_map();
        let cat = Entity::from_raw_u32(1).unwrap();
        // Equal-Manhattan-distance Bloom-stage herbs of the same kind,
        // placed in non-overlapping buckets:
        //   dense → (5, 5), bucket (1, 1) center (7, 7)
        //   lone  → (10, 0), bucket (2, 0) center (12, 2)
        // Bucket-center distance ≈ 7.07, so a stamp range of 5.0 stays
        // local — `dense`'s thickening doesn't bleed into `lone`'s
        // bucket and vice versa.
        let dense = herb(2, 5, 5, HerbKind::Thornbriar, GrowthStage::Bloom);
        let lone = herb(3, 10, 0, HerbKind::Thornbriar, GrowthStage::Bloom);
        // Multiple stamps at `dense`'s position saturate its bucket
        // (each contributes ~0.43 at the bucket center; three stamps
        // cap at 1.0).
        for _ in 0..3 {
            map.stamp(dense.kind, dense.position.x, dense.position.y, 1.0, 5.0);
        }
        map.stamp(lone.kind, lone.position.x, lone.position.y, 0.5, 5.0);
        assert!(
            map.total(dense.position.x, dense.position.y)
                > map.total(lone.position.x, lone.position.y) + 0.2,
            "test setup must produce a density gap; got {} vs {}",
            map.total(dense.position.x, dense.position.y),
            map.total(lone.position.x, lone.position.y),
        );
        let out = resolve_herbcraft_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[dense, lone],
            &map,
            0,
            None,
        );
        assert_eq!(out, Some(dense.entity));
    }
}
