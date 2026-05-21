//! Ticket 035 — `BuryTargetDse`. Pairs with the self-state
//! [`bury_dse`](super::bury::bury_dse) which decides *whether* to
//! bury; this DSE decides *which* corpse.
//!
//! The candidate set is built upstream (in `goap.rs::ScoringSnapshots`)
//! from `Query<(Entity, &Position), (With<Dead>, Without<Buried>)>`;
//! the DSE's per-target considerations rank by adjacency, bond to the
//! deceased, kinship, and recent-target-failure cooldown. No
//! `require_alive_filter` on the DSE itself — the candidate set is
//! the gate (we *want* dead candidates).
//!
//! | # | Consideration       | Source                        | Curve                                 | Weight |
//! |---|---------------------|-------------------------------|---------------------------------------|--------|
//! | 1 | distance            | `Spatial(target)`             | `Composite{Logistic(15, 0.15), Invert}` | 0.30   |
//! | 2 | fondness            | `target_fondness`             | `Linear(1.0, 0.3)`                    | 0.30   |
//! | 3 | kinship             | `target_kinship`              | `Piecewise` (kin=1.0 / else=0.5)      | 0.10   |
//! | 4 | recent-failure      | `target_recent_failure`       | `cooldown_curve()`                    | 0.30   |
//!
//! **Bond curve.** `Linear(1.0, 0.3)` — the intercept-0.3 floor lets
//! a cat with no recorded relationship to the deceased still bury
//! (community duty), while a strongly-bonded cat scores higher.
//! Mirrors grooming's social-warmth-deficit floor at 0.1, lifted a
//! bit because relationships persist after death and the typical
//! cat will have at least some fondness for a colony-mate.
//!
//! **Distance curve.** Same algebra as grooming's
//! `Composite{Logistic(15, 0.15), Invert}` over normalized cost
//! (`dist / BURY_TARGET_RANGE`). Saturates near adjacency, drops
//! below 0.1 by ~3 tiles. Burial is a physical act at the corpse
//! tile; the cat needs to be near to do it.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
use crate::ai::eval::DseRegistry;
use crate::ai::planner::GoapActionKind;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::components::RecentTargetFailures;
use crate::resources::relationships::Relationships;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::systems::plan_substrate::{
    cooldown_curve, target_recent_failure_age_normalized, TARGET_RECENT_FAILURE_INPUT,
};

pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";
pub const TARGET_KINSHIP_INPUT: &str = "target_kinship";

/// Candidate-pool range in Manhattan tiles. Matches the
/// `burial_sense_range` constant authored by sensing — same range gates
/// the `HasUnburiedCorpse` marker, so cats only ever pick burial
/// targets they would have scored as eligible.
pub const BURY_TARGET_RANGE: f32 = 8.0;

pub fn bury_target_dse() -> TargetTakingDse {
    let bond_curve = Curve::Linear {
        slope: 1.0,
        intercept: 0.3,
    };
    // Same algebra as grooming's adjacency-saturating curve. Crosses
    // 0.5 at dist ≈ 1.2 (with range=8 the inverted midpoint at
    // cost=0.15 → dist=1.2), drops below 0.1 by dist=2.5.
    let nearness_curve = Curve::Composite {
        inner: Box::new(Curve::Logistic {
            steepness: 15.0,
            midpoint: 0.15,
        }),
        post: PostOp::Invert,
    };
    let kinship_curve = Curve::Piecewise {
        knots: vec![(0.0, 0.5), (0.999, 0.5), (1.0, 1.0)],
    };

    TargetTakingDse {
        id: DseId("bury_target"),
        candidate_query: bury_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Spatial(SpatialConsideration::new(
                "bury_target_nearness",
                LandmarkSource::TargetPosition,
                BURY_TARGET_RANGE,
                nearness_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(TARGET_FONDNESS_INPUT, bond_curve)),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_KINSHIP_INPUT,
                kinship_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_RECENT_FAILURE_INPUT,
                cooldown_curve(),
            )),
        ],
        // Renormalized weights summing to 1.0: 0.30 / 0.30 / 0.10 /
        // 0.30 (= 1.0). Same total share as grooming's four-axis
        // composition, redistributed because there's no
        // target-warmth-deficit axis (the deceased has no needs).
        composition: Composition::weighted_sum(vec![0.30, 0.30, 0.10, 0.30]),
        aggregation: TargetAggregation::Best,
        intention: bury_intention,
        required_stance: None,
        // No alive filter — burial targets are deliberately Dead.
        // The candidate set built upstream (Dead, Without<Buried>)
        // is the gate.
        eligibility: Default::default(),
    }
}

fn bury_candidate_query_doc(_cat: Entity) -> &'static str {
    "Dead-and-not-Buried entities within BURY_TARGET_RANGE Manhattan, no other filter"
}

fn bury_intention(_target: Entity) -> Intention {
    Intention::Activity {
        kind: ActivityKind::Bury,
        termination: Termination::UntilInterrupt,
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best burial target for `cat` from the dead-cat snapshot.
/// Returns `None` iff no Dead-and-not-Buried candidate exists in
/// `BURY_TARGET_RANGE`.
///
/// The caller (`goap.rs::resolve_goap_plans`'s `Bury` dispatch arm)
/// supplies:
/// - `dead_cat_positions` — frame-local snapshot of `(Entity, Position)`
///   pairs filtered `(With<Dead>, Without<Buried>)`.
/// - `is_kin(self, deceased)` — bidirectional parent-child check via
///   `KittenDependency.mother / .father`. Same lookup grooming uses;
///   reusable closure built once per tick.
#[allow(clippy::too_many_arguments)]
pub fn resolve_bury_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    dead_cat_positions: &[(Entity, Position)],
    is_kin: &dyn Fn(Entity, Entity) -> bool,
    relationships: &Relationships,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
    recent: Option<&RecentTargetFailures>,
    cooldown_ticks: u64,
    activation: Option<&mut SystemActivation>,
    // Ticket 427 Step 1 — pre-allocated scratch buffers.
    scratch: &mut crate::resources::DseTargetScratchpad,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "bury_target")?;

    scratch.entities.clear();
    scratch.positions.clear();
    for (other, other_pos) in dead_cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos) as f32;
        if dist > BURY_TARGET_RANGE {
            continue;
        }
        scratch.entities.push(*other);
        scratch.positions.push(*other_pos);
    }

    if scratch.entities.is_empty() {
        return None;
    }

    let cooldown_was_applied = std::cell::Cell::new(false);
    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
                .unwrap_or(0.0),
            TARGET_KINSHIP_INPUT if is_kin(cat, target) => 1.0,
            TARGET_RECENT_FAILURE_INPUT => {
                let signal = target_recent_failure_age_normalized(
                    recent,
                    GoapActionKind::Bury,
                    target,
                    tick,
                    cooldown_ticks,
                );
                if signal < 1.0 {
                    cooldown_was_applied.set(true);
                }
                signal
            }
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
        target_alive: None,
        field_cost: None,
    };

    let scored = evaluate_target_taking(
        dse,
        cat,
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
            hook.capture
                .set_target_ranking("bury_target", ranking, tick);
        }
    }

    if let Some(act) = activation {
        if cooldown_was_applied.get() {
            act.record(Feature::TargetCooldownApplied);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bury_target_dse_id_stable() {
        assert_eq!(bury_target_dse().id().0, "bury_target");
    }

    #[test]
    fn bury_target_has_four_axes() {
        assert_eq!(bury_target_dse().per_target_considerations().len(), 4);
    }

    #[test]
    fn bury_target_weights_sum_to_one() {
        let sum: f32 = bury_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "got {sum}");
    }

    #[test]
    fn bury_target_uses_best_aggregation() {
        assert_eq!(bury_target_dse().aggregation(), TargetAggregation::Best);
    }

    #[test]
    fn intention_is_bury_activity() {
        let dse = bury_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Activity { kind, .. } => assert_eq!(kind, ActivityKind::Bury),
            other => panic!("expected Activity intention, got {other:?}"),
        }
    }

    #[test]
    fn resolver_returns_none_with_no_candidates() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(bury_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let is_kin = |_: Entity, _: Entity| false;
        let out = resolve_bury_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            &is_kin,
            &relationships,
            0,
            None,
            None,
            8000,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_excludes_self() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(bury_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let is_kin = |_: Entity, _: Entity| false;
        let out = resolve_bury_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[(cat, Position::new(0, 0))],
            &is_kin,
            &relationships,
            0,
            None,
            None,
            8000,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_picks_nearest_when_bonds_tied() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(bury_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let near = Entity::from_raw_u32(2).unwrap();
        let far = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, near).fondness = 0.5;
        relationships.get_or_insert(cat, far).fondness = 0.5;
        let is_kin = |_: Entity, _: Entity| false;
        let dead_positions = vec![(near, Position::new(1, 0)), (far, Position::new(5, 0))];
        let out = resolve_bury_target(
            &registry,
            cat,
            Position::new(0, 0),
            &dead_positions,
            &is_kin,
            &relationships,
            0,
            None,
            None,
            8000,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(near));
    }

    #[test]
    fn resolver_kin_beats_non_kin_at_equal_distance() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(bury_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let kin = Entity::from_raw_u32(2).unwrap();
        let stranger = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, kin).fondness = 0.5;
        relationships.get_or_insert(cat, stranger).fondness = 0.5;
        let is_kin = move |_: Entity, target: Entity| target == kin;
        let dead_positions = vec![(kin, Position::new(1, 0)), (stranger, Position::new(0, 1))];
        let out = resolve_bury_target(
            &registry,
            cat,
            Position::new(0, 0),
            &dead_positions,
            &is_kin,
            &relationships,
            0,
            None,
            None,
            8000,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(kin));
    }

    #[test]
    fn resolver_drops_candidates_outside_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(bury_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        let is_kin = |_: Entity, _: Entity| false;
        let dead_positions = vec![(far, Position::new(50, 0))];
        let out = resolve_bury_target(
            &registry,
            cat,
            Position::new(0, 0),
            &dead_positions,
            &is_kin,
            &relationships,
            0,
            None,
            None,
            8000,
            None,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }
}
