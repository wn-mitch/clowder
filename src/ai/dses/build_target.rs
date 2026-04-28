//! `BuildTargetDse` — §6.5.8 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning work-site selection for `Build`. Pairs
//! with the self-state [`BuildDse`](super::build::build_dse) which
//! decides *whether* to build; this DSE decides *which site*.
//!
//! Phase 4c.10 scope — progress-urgency + structural-condition
//! awareness. The legacy picker in
//! `disposition.rs::build_building_chain` used
//! `min_by_key((priority, dist))` where `priority = 0` for
//! ConstructionSites and `1` for repair-needed Structures — near-
//! flat distance ranking inside each priority class. Two spec axes
//! were §6.1 Partial-invisible:
//!
//! - **progress-urgency:** a construction site at 95% complete is
//!   worth more attention than one at 10% — the "sunk-progress"
//!   effect the spec calls out. Quadratic(2) amplifies high-progress
//!   sites sharply.
//! - **structural-condition:** a damaged Structure at condition=0.3
//!   ranks higher than one at condition=0.9 (both need repair).
//!   Linear inversion turns condition into urgency.
//!
//! Four per-target considerations per §6.5.8. All four spec axes
//! port directly; aggregation is `Best`.
//!
//! | # | Consideration         | Scalar name              | Curve                  | Weight |
//! |---|-----------------------|--------------------------|------------------------|--------|
//! | 1 | distance              | `target_nearness`        | `Linear(1, 0)`         | 0.20   |
//! | 2 | site-type             | `target_site_type`       | Piecewise cliff        | 0.30   |
//! | 3 | progress-urgency      | `target_progress_urgency`| `Quadratic(exp=2)`     | 0.30   |
//! | 4 | structural-condition  | `target_condition_urgency`| `Linear(1, 0)` on deficit | 0.20 |
//!
//! **Distance curve reinterpretation.** Spec row #8 says
//! `Linear(slope=-1/20, intercept=1), range=20` — literally a line
//! from (dist=0 → 1) to (dist=20 → 0). Mapped to the normalized
//! `1 − dist/range` signal the DSE substrate uses, this is exactly
//! `Linear(1, 0)` — pass-through. The resolver computes the
//! normalization upstream so the curve stays uniform with the other
//! target DSEs.
//!
//! **Site-type encoding.** A candidate is either a ConstructionSite
//! (scalar=1.0) or a needs-repair Structure (scalar=0.6). The
//! Piecewise cliff at 0.5 reifies the spec row's "Cliff
//! (ConstructionSite=1.0, RepairNeeded=0.6)" without needing a
//! custom curve — signal ≥ 0.5 outputs its value directly.
//!
//! **Progress-urgency.** `ConstructionSite.progress` ∈ [0, 1]. For
//! a repair target (no ConstructionSite), the signal is 0 — sunk-
//! progress doesn't apply. Quadratic(2) amplifies near-complete
//! sites: progress=0.9 → 0.81; progress=0.3 → 0.09.
//!
//! **Structural-condition urgency.** `1 − Structure.condition` —
//! the deficit form. For a new ConstructionSite (no Structure yet
//! in broken state), the signal is 0. For a repair target with
//! condition=0.3, signal is 0.7.

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

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const TARGET_SITE_TYPE_INPUT: &str = "target_site_type";
pub const TARGET_PROGRESS_URGENCY_INPUT: &str = "target_progress_urgency";
pub const TARGET_CONDITION_URGENCY_INPUT: &str = "target_condition_urgency";

/// Candidate-pool range in Manhattan tiles. Matches spec §6.4 row #8
/// — builders cross the colony for priority sites.
pub const BUILD_TARGET_RANGE: f32 = 20.0;

/// What kind of work the target needs. `NewBuild` takes a fresh
/// ConstructionSite; `Repair` takes a damaged Structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTargetKind {
    NewBuild,
    Repair,
}

/// Per-site snapshot fed to `resolve_build_target`.
#[derive(Clone, Copy, Debug)]
pub struct BuildCandidate {
    pub entity: Entity,
    pub position: Position,
    pub kind: BuildTargetKind,
    /// For `NewBuild`: the `ConstructionSite.progress` ∈ [0, 1].
    /// For `Repair`: 0 (sunk-progress doesn't apply).
    pub progress: f32,
    /// For `Repair`: the `Structure.condition` ∈ [0, 1]. Used to
    /// compute `1 − condition` as the urgency signal.
    /// For `NewBuild`: 1.0 (no structural damage — the
    /// condition-urgency signal will be 0).
    pub condition: f32,
}

/// §6.5.8 `Build` target-taking DSE factory.
pub fn build_target_dse() -> TargetTakingDse {
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    let progress_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    // Piecewise cliff: signal=1.0 (ConstructionSite) → 1.0, signal=0.6
    // (Repair) → 0.6, signal=0.0 → 0.0. Matches spec §6.5.8 row #2.
    let site_type_curve = Curve::Piecewise {
        knots: vec![
            (0.0, 0.0),
            (0.59, 0.0),
            (0.6, 0.6),
            (0.999, 0.6),
            (1.0, 1.0),
        ],
    };

    TargetTakingDse {
        id: DseId("build_target"),
        candidate_query: build_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_NEARNESS_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_SITE_TYPE_INPUT,
                site_type_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_PROGRESS_URGENCY_INPUT,
                progress_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_CONDITION_URGENCY_INPUT,
                linear,
            )),
        ],
        composition: Composition::weighted_sum(vec![0.20, 0.30, 0.30, 0.20]),
        aggregation: TargetAggregation::Best,
        intention: build_intention,
        required_stance: None,
    }
}

fn build_candidate_query_doc(_cat: Entity) -> &'static str {
    "construction sites + needs-repair structures within BUILD_TARGET_RANGE"
}

fn build_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "site_completed",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best work-site for `cat` via the registered
/// [`build_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
pub fn resolve_build_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    candidates: &[BuildCandidate],
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "build_target")?;

    if candidates.is_empty() {
        return None;
    }

    let mut entities: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    let mut kind_map: std::collections::HashMap<Entity, BuildTargetKind> =
        std::collections::HashMap::new();
    let mut progress_map: std::collections::HashMap<Entity, f32> = std::collections::HashMap::new();
    let mut condition_map: std::collections::HashMap<Entity, f32> =
        std::collections::HashMap::new();
    for c in candidates {
        let dist = cat_pos.manhattan_distance(&c.position) as f32;
        if dist > BUILD_TARGET_RANGE {
            continue;
        }
        entities.push(c.entity);
        positions.push(c.position);
        kind_map.insert(c.entity, c.kind);
        progress_map.insert(c.entity, c.progress.clamp(0.0, 1.0));
        condition_map.insert(c.entity, c.condition.clamp(0.0, 1.0));
    }

    if entities.is_empty() {
        return None;
    }

    let pos_map: std::collections::HashMap<Entity, Position> = entities
        .iter()
        .copied()
        .zip(positions.iter().copied())
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
                (1.0 - dist / BUILD_TARGET_RANGE).clamp(0.0, 1.0)
            }
            TARGET_SITE_TYPE_INPUT => match kind_map.get(&target) {
                Some(BuildTargetKind::NewBuild) => 1.0,
                Some(BuildTargetKind::Repair) => 0.6,
                None => 0.0,
            },
            TARGET_PROGRESS_URGENCY_INPUT => {
                match kind_map.get(&target) {
                    Some(BuildTargetKind::NewBuild) => {
                        progress_map.get(&target).copied().unwrap_or(0.0)
                    }
                    // Repair has no sunk-progress — this axis is 0.
                    _ => 0.0,
                }
            }
            TARGET_CONDITION_URGENCY_INPUT => {
                match kind_map.get(&target) {
                    Some(BuildTargetKind::Repair) => condition_map
                        .get(&target)
                        .map(|c| (1.0 - c).clamp(0.0, 1.0))
                        .unwrap_or(0.0),
                    // NewBuild has no structural damage — this axis is 0.
                    _ => 0.0,
                }
            }
            _ => 0.0,
        }
    };

    let entity_position = |_: Entity| -> Option<Position> { None };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat,
        tick,
        entity_position: &entity_position,
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
                .set_target_ranking("build_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_build(entity_id: u32, x: i32, y: i32, progress: f32) -> BuildCandidate {
        BuildCandidate {
            entity: Entity::from_raw_u32(entity_id).unwrap(),
            position: Position::new(x, y),
            kind: BuildTargetKind::NewBuild,
            progress,
            condition: 1.0,
        }
    }

    fn repair(entity_id: u32, x: i32, y: i32, condition: f32) -> BuildCandidate {
        BuildCandidate {
            entity: Entity::from_raw_u32(entity_id).unwrap(),
            position: Position::new(x, y),
            kind: BuildTargetKind::Repair,
            progress: 0.0,
            condition,
        }
    }

    #[test]
    fn build_target_dse_id_stable() {
        assert_eq!(build_target_dse().id().0, "build_target");
    }

    #[test]
    fn build_target_has_four_axes() {
        assert_eq!(build_target_dse().per_target_considerations().len(), 4);
    }

    #[test]
    fn build_target_weights_sum_to_one() {
        let sum: f32 = build_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn build_target_uses_best_aggregation() {
        assert_eq!(build_target_dse().aggregation(), TargetAggregation::Best);
    }

    #[test]
    fn intention_is_site_completed_goal() {
        let dse = build_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label, "site_completed");
                assert_eq!(strategy, CommitmentStrategy::SingleMinded);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_build_target(&registry, cat, Position::new(0, 0), &[], 0, None);
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_with_empty_candidates() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(build_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_build_target(&registry, cat, Position::new(0, 0), &[], 0, None);
        assert!(out.is_none());
    }

    #[test]
    fn resolver_filters_out_of_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(build_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = new_build(2, 50, 0, 0.5);
        let out = resolve_build_target(&registry, cat, Position::new(0, 0), &[far], 0, None);
        assert!(out.is_none());
    }

    #[test]
    fn new_build_beats_repair_at_equal_distance() {
        // Site-type Cliff: NewBuild (1.0) > Repair (0.6). When other
        // axes tie, the Cliff decides — matches legacy priority ordering.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(build_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let nb = new_build(2, 3, 0, 0.5);
        // Repair with moderate urgency (condition=0.5 → deficit=0.5)
        // vs new-build with moderate progress (0.5 → 0.25 quadratic).
        // Let's make their progress/condition axes tie to isolate the
        // Cliff's effect.
        let rp = repair(3, 0, 3, 0.5);
        let out = resolve_build_target(&registry, cat, Position::new(0, 0), &[nb, rp], 0, None);
        assert_eq!(out, Some(nb.entity));
    }

    #[test]
    fn nearly_complete_site_wins_over_fresh_site() {
        // Quadratic amplification: progress=0.9 → 0.81; progress=0.1
        // → 0.01. At equal distance the nearly-done site dominates.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(build_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let finishing = new_build(2, 2, 0, 0.9);
        let breaking_ground = new_build(3, 0, 2, 0.1);
        let out = resolve_build_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[finishing, breaking_ground],
            0,
            None,
        );
        assert_eq!(out, Some(finishing.entity));
    }

    #[test]
    fn heavily_damaged_repair_wins_over_lightly_damaged() {
        // Structural-condition inversion: condition=0.2 → deficit=0.8;
        // condition=0.8 → deficit=0.2. At equal distance, deficit 0.8
        // dominates.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(build_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let crumbling = repair(2, 2, 0, 0.2);
        let scuffed = repair(3, 0, 2, 0.8);
        let out = resolve_build_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[crumbling, scuffed],
            0,
            None,
        );
        assert_eq!(out, Some(crumbling.entity));
    }

    #[test]
    fn close_site_outscores_distant_similar_site() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(build_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = new_build(2, 2, 0, 0.5);
        let far = new_build(3, 15, 0, 0.5);
        let out = resolve_build_target(&registry, cat, Position::new(0, 0), &[close, far], 0, None);
        assert_eq!(out, Some(close.entity));
    }

    #[test]
    fn nearly_complete_site_beats_heavy_repair() {
        // Finishing a 95%-done build (progress=0.95 → 0.9025
        // urgency) beats repairing a half-broken building
        // (condition=0.5 → 0.5 deficit). At equal distance and
        // site-type-weight-tied setups, the higher-urgency site wins.
        //
        // Scores:
        //   finishing: nearness 0.9*0.2 = 0.18
        //              site_type (new) 1.0*0.3 = 0.30
        //              progress 0.9025*0.3 = 0.27
        //              condition 0.0*0.2 = 0.0
        //              total ≈ 0.75
        //   damaged:   nearness 0.9*0.2 = 0.18
        //              site_type (repair) 0.6*0.3 = 0.18
        //              progress 0.0*0.3 = 0.0
        //              condition 0.5*0.2 = 0.10
        //              total ≈ 0.46
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(build_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let finishing = new_build(2, 2, 0, 0.95);
        let damaged = repair(3, 0, 2, 0.5);
        let out = resolve_build_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[finishing, damaged],
            0,
            None,
        );
        assert_eq!(out, Some(finishing.entity));
    }
}
