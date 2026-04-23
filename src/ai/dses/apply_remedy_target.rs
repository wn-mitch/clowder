//! `ApplyRemedyTargetDse` — §6.5.7 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning patient selection for `ApplyRemedy`. The
//! action itself is surfaced through Herbcraft's `PrepareRemedy`
//! chain — this DSE picks *whom the poultice heals*, replacing the
//! legacy `injured_cats.iter().min_by_key(distance)` pick at
//! `disposition.rs::try_crafting_sub_mode::PrepareRemedy`.
//!
//! Phase 4c.9 scope — severity-aware triage. The legacy picker
//! chose nearest patient regardless of injury severity, so a cat at
//! health=0.95 next door would be preferred over a cat at health=0.3
//! across the colony. §6.1 Partial fix: the DSE scores distance,
//! injury severity, and kinship together; severity dominates via its
//! Quadratic amplification.
//!
//! Three per-target considerations per §6.5.7. The `remedy-match`
//! axis (Cliff gating HealingPoultice vs. mood-injury-remedy) is
//! deferred — remedies today are effectively single-class (the
//! HealingPoultice/EnergyTonic/MoodTonic switch in
//! `build_crafting_chain::PrepareRemedy` is remedy-kind selection
//! *at prepare time*, not a per-candidate match). Weights
//! renormalized from the spec's (0.15/0.40/0.30/0.15) by dropping
//! the 0.30 remedy-match row and dividing by 0.70:
//!
//! | # | Consideration      | Scalar name           | Curve                    | Spec weight | Renormalized |
//! |---|--------------------|-----------------------|--------------------------|-------------|--------------|
//! | 1 | distance           | `target_nearness`     | `Quadratic(exp=1.5)`     | 0.15        | 0.214        |
//! | 2 | injury-severity    | `target_injury`       | `Quadratic(exp=2)`       | 0.40        | 0.571        |
//! | 3 | kinship            | `target_kinship`      | `Linear(0.5, 0.5)`       | 0.15        | 0.214        |
//!
//! **Distance curve.** Spec §6.4 row #7 specifies `Quadratic(exp=1.5),
//! range=15`. The 1.5 exponent sits between linear falloff and the
//! stronger Quadratic(2) used for adjacency-sensitive DSEs —
//! healers cross the colony, but a patient at range 15 still
//! deserves less attention than one at range 3.
//!
//! **Injury severity.** `1 − health.current / health.max` — the
//! standard deficit axis. Convex Quadratic amplifies desperate
//! need; a patient at health=0.3 (deficit=0.7) contributes ~0.49,
//! while a patient at health=0.95 (deficit=0.05) contributes
//! ~0.003. This is the axis the legacy picker could not see.
//!
//! **Kinship.** Linear(0.5, 0.5) per spec — non-kin scores 0.5
//! (signal=0), kin scores 1.0 (signal=1). Mild bias; the weight
//! (0.214) is intentionally small so colony-wide healing remains
//! the norm.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{evaluate_target_taking, TargetAggregation, TargetTakingDse};
use crate::components::physical::Position;

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const TARGET_INJURY_INPUT: &str = "target_injury";
pub const TARGET_KINSHIP_INPUT: &str = "target_kinship";

/// Candidate-pool range in Manhattan tiles. Matches spec §6.4 row #7
/// — healers cross the colony for severe injury. Outer cutoff
/// beyond which the caller doesn't bother building a candidate
/// snapshot.
pub const APPLY_REMEDY_TARGET_RANGE: f32 = 15.0;

/// Per-patient snapshot fed to `resolve_apply_remedy_target`.
#[derive(Clone, Copy, Debug)]
pub struct PatientCandidate {
    pub entity: Entity,
    pub position: Position,
    /// `health.current / health.max` — clamped to [0, 1].
    pub health_fraction: f32,
}

/// §6.5.7 `ApplyRemedy` target-taking DSE factory.
pub fn apply_remedy_target_dse() -> TargetTakingDse {
    let nearness_curve = Curve::Quadratic {
        exponent: 1.5,
        divisor: 1.0,
        shift: 0.0,
    };
    let injury_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    // Kinship Linear(0.5, 0.5) — signal ∈ {0.0, 1.0}, curve output
    // ∈ {0.5, 1.0}. Non-kin still attended; kin biased mildly.
    let kinship_curve = Curve::Linear {
        slope: 0.5,
        intercept: 0.5,
    };

    TargetTakingDse {
        id: DseId("apply_remedy_target"),
        candidate_query: apply_remedy_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(TARGET_NEARNESS_INPUT, nearness_curve)),
            Consideration::Scalar(ScalarConsideration::new(TARGET_INJURY_INPUT, injury_curve)),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_KINSHIP_INPUT,
                kinship_curve,
            )),
        ],
        // Weights are `[3, 8, 3] / 14` — the spec-renormalized
        // distribution computed to f32 precision so the RtEO
        // invariant sum-to-1.0 assertion in `Composition::compose`
        // holds.
        composition: Composition::weighted_sum(vec![3.0 / 14.0, 8.0 / 14.0, 3.0 / 14.0]),
        aggregation: TargetAggregation::Best,
        intention: apply_remedy_intention,
    }
}

fn apply_remedy_candidate_query_doc(_cat: Entity) -> &'static str {
    "injured cats within APPLY_REMEDY_TARGET_RANGE (health.current < health.max)"
}

fn apply_remedy_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState {
            label: "injury_healed",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best patient for `cat` via the registered
/// [`apply_remedy_target_dse`]. Returns `None` iff no eligible
/// candidate exists in range.
///
/// - `candidates` is the caller-built injured-cat snapshot.
/// - `is_kin(self, target)` — parent-child check (same shape as
///   Groom-other §6.5.4).
pub fn resolve_apply_remedy_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    candidates: &[PatientCandidate],
    is_kin: &dyn Fn(Entity, Entity) -> bool,
    tick: u64,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "apply_remedy_target")?;

    if candidates.is_empty() {
        return None;
    }

    let mut entities: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    let mut injury_map: std::collections::HashMap<Entity, f32> = std::collections::HashMap::new();
    for c in candidates {
        let dist = cat_pos.manhattan_distance(&c.position) as f32;
        if dist > APPLY_REMEDY_TARGET_RANGE {
            continue;
        }
        entities.push(c.entity);
        positions.push(c.position);
        injury_map.insert(c.entity, (1.0 - c.health_fraction).clamp(0.0, 1.0));
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
    let fetch_target = |name: &str, cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_NEARNESS_INPUT => {
                let target_pos = match pos_map.get(&target) {
                    Some(p) => *p,
                    None => return 0.0,
                };
                let dist = cat_pos.manhattan_distance(&target_pos) as f32;
                (1.0 - dist / APPLY_REMEDY_TARGET_RANGE).clamp(0.0, 1.0)
            }
            TARGET_INJURY_INPUT => injury_map.get(&target).copied().unwrap_or(0.0),
            TARGET_KINSHIP_INPUT => {
                if is_kin(cat, target) {
                    1.0
                } else {
                    0.0
                }
            }
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

    evaluate_target_taking(
        dse,
        cat,
        &entities,
        &positions,
        &ctx,
        &fetch_self,
        &fetch_target,
    )
    .winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patient(entity_id: u32, x: i32, y: i32, health_fraction: f32) -> PatientCandidate {
        PatientCandidate {
            entity: Entity::from_raw_u32(entity_id).unwrap(),
            position: Position::new(x, y),
            health_fraction,
        }
    }

    #[test]
    fn apply_remedy_target_dse_id_stable() {
        assert_eq!(apply_remedy_target_dse().id().0, "apply_remedy_target");
    }

    #[test]
    fn apply_remedy_target_has_three_axes() {
        assert_eq!(
            apply_remedy_target_dse().per_target_considerations().len(),
            3
        );
    }

    #[test]
    fn apply_remedy_target_weights_sum_to_one() {
        let sum: f32 = apply_remedy_target_dse()
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn apply_remedy_target_uses_best_aggregation() {
        assert_eq!(
            apply_remedy_target_dse().aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn intention_is_injury_healed_goal() {
        let dse = apply_remedy_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label, "injury_healed");
                assert_eq!(strategy, CommitmentStrategy::SingleMinded);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    #[test]
    fn resolver_returns_none_with_empty_candidates() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            &is_kin,
            0,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_filters_out_of_range() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = patient(2, 50, 0, 0.3);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[far],
            &is_kin,
            0,
        );
        assert!(out.is_none());
    }

    #[test]
    fn picks_more_injured_at_equal_distance() {
        // §6.1 Partial fix: severe patient (health=0.3, deficit=0.7)
        // wins over light-injury patient (health=0.95, deficit=0.05)
        // at equal distance. Weight ratio + Quadratic amplification
        // decides decisively.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let severe = patient(2, 3, 0, 0.3);
        let mild = patient(3, 0, 3, 0.95);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[severe, mild],
            &is_kin,
            0,
        );
        assert_eq!(out, Some(severe.entity));
    }

    #[test]
    fn severity_dominates_distance() {
        // A cat at health=0.2 (deficit=0.8, injury curve output ≈
        // 0.64) across the colony at dist=10 still beats a cat at
        // health=0.9 (deficit=0.1, injury curve ≈ 0.01) nearby at
        // dist=1, because severity's weight (0.571) × 0.64 ≈ 0.366
        // dominates nearness's weight (0.214) × (nearness
        // contribution at dist=10 range=15 ≈ 0.33²·⁵ ≈ 0.06) ≈ 0.013.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let critical_far = patient(2, 10, 0, 0.2);
        let mild_near = patient(3, 1, 0, 0.9);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[critical_far, mild_near],
            &is_kin,
            0,
        );
        assert_eq!(out, Some(critical_far.entity));
    }

    #[test]
    fn close_patient_outscores_distant_same_injury() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = patient(2, 2, 0, 0.5);
        let far = patient(3, 10, 0, 0.5);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[close, far],
            &is_kin,
            0,
        );
        assert_eq!(out, Some(close.entity));
    }

    #[test]
    fn kin_beats_non_kin_when_other_axes_tied() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let kin = patient(2, 3, 0, 0.5);
        let stranger = patient(3, 0, 3, 0.5);
        let kin_e = kin.entity;
        let is_kin = move |_: Entity, target: Entity| -> bool { target == kin_e };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[kin, stranger],
            &is_kin,
            0,
        );
        assert_eq!(out, Some(kin.entity));
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let patient = patient(2, 1, 0, 0.5);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[patient],
            &is_kin,
            0,
        );
        assert!(out.is_none());
    }
}
