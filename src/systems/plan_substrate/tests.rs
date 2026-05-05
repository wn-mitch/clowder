//! Per-call-site equivalence tests for ticket 072.
//!
//! For each migration site, we build a fixed input, run **the new
//! `plan_substrate` API** and **the old inline body** side by side,
//! then assert they produce identical mutations. These tests gate the
//! refactor: any divergence here means the bit-identical-footer soak
//! gate would fail.

use bevy_ecs::prelude::*;
use std::collections::HashSet;

use crate::ai::planner::{GoapActionKind, PlannedStep, PlannerZone};
use crate::ai::{Action, CurrentAction};
use crate::components::disposition::{Disposition, DispositionKind};
use crate::components::goap_plan::{AbandonReason, PlanFailureReason, StepExecutionState};
use crate::components::personality::Personality;
use crate::components::physical::Position;
use crate::components::GoapPlan;

use super::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn test_personality() -> Personality {
    Personality {
        boldness: 0.5,
        sociability: 0.5,
        curiosity: 0.5,
        diligence: 0.5,
        warmth: 0.5,
        spirituality: 0.5,
        ambition: 0.5,
        patience: 0.5,
        anxiety: 0.5,
        optimism: 0.5,
        temper: 0.5,
        stubbornness: 0.5,
        playfulness: 0.5,
        loyalty: 0.5,
        tradition: 0.5,
        compassion: 0.5,
        pride: 0.5,
        independence: 0.5,
    }
}

fn sample_steps() -> Vec<PlannedStep> {
    vec![
        PlannedStep {
            action: GoapActionKind::TravelTo(PlannerZone::HuntingGround),
            cost: 3,
        },
        PlannedStep {
            action: GoapActionKind::SearchPrey,
            cost: 3,
        },
        PlannedStep {
            action: GoapActionKind::EngagePrey,
            cost: 2,
        },
    ]
}

fn fresh_plan() -> GoapPlan {
    GoapPlan::new(
        DispositionKind::Hunting,
        crate::ai::Action::Hunt,
        100,
        &test_personality(),
        sample_steps(),
    )
}

fn fresh_current() -> CurrentAction {
    CurrentAction {
        action: Action::Hunt,
        ticks_remaining: u64::MAX,
        target_position: None,
        target_entity: None,
        last_scores: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// record_step_failure — `goap.rs:2451–2455`
// ---------------------------------------------------------------------------

#[test]
fn record_step_failure_matches_inline_body() {
    let mut new_plan = fresh_plan();
    let mut old_plan = fresh_plan();
    let action = GoapActionKind::SearchPrey;

    // Inline body (verbatim from goap.rs):
    old_plan.failed_actions.insert(action);

    // New API:
    record_step_failure(
        &mut new_plan,
        action,
        PlanFailureReason::Other,
        None,
        None,
        0,
    );

    assert_eq!(new_plan.failed_actions, old_plan.failed_actions);
    let mut expected = HashSet::new();
    expected.insert(GoapActionKind::SearchPrey);
    assert_eq!(new_plan.failed_actions, expected);
}

#[test]
fn record_step_failure_preserves_existing_entries() {
    let mut plan = fresh_plan();
    plan.failed_actions.insert(GoapActionKind::ForageItem);

    record_step_failure(
        &mut plan,
        GoapActionKind::SearchPrey,
        PlanFailureReason::Other,
        None,
        None,
        0,
    );

    assert!(plan.failed_actions.contains(&GoapActionKind::ForageItem));
    assert!(plan.failed_actions.contains(&GoapActionKind::SearchPrey));
}

// ---------------------------------------------------------------------------
// abandon_plan — `goap.rs:2574` and `goap.rs:2593`
// ---------------------------------------------------------------------------

#[test]
fn abandon_plan_matches_inline_body() {
    let mut new_current = fresh_current();
    let mut new_plan = fresh_plan();
    let mut old_current = fresh_current();
    let _old_plan = fresh_plan();

    // Inline body (verbatim from goap.rs:2574 / :2593):
    old_current.ticks_remaining = 0;
    // (caller pushes cat onto plans_to_remove — substrate doesn't own that list)

    // New API:
    let state = abandon_plan(
        &mut new_current,
        &mut new_plan,
        AbandonReason::ReplanCap,
        None,
        None,
        None,
        0,
    );

    assert_eq!(new_current.ticks_remaining, old_current.ticks_remaining);
    assert_eq!(new_current.action, old_current.action);
    assert_eq!(new_current.target_position, old_current.target_position);
    // AbandonedPlanState is empty in 072 — nothing to assert on the
    // returned snapshot. 073 extends it with cross-plan target memory.
    let _ = state;
}

// ---------------------------------------------------------------------------
// try_preempt — `goap.rs:2342–2401` (ThreatNearby + non-threat branches)
// ---------------------------------------------------------------------------

#[test]
fn try_preempt_threat_flee_matches_inline_body() {
    let mut new_plan = fresh_plan();
    let mut new_current = fresh_current();
    let mut old_plan = fresh_plan();
    let mut old_current = fresh_current();

    let flee_target = Position::new(42, 17);

    // Inline body (verbatim from goap.rs:2342–2401, ThreatNearby branch):
    old_current.action = Action::Flee;
    old_current.ticks_remaining = 0;
    old_current.target_position = Some(flee_target);
    old_current.target_entity = None;
    old_plan.current_step = old_plan.steps.len();
    old_current.ticks_remaining = 0;

    // New API:
    let outcome = try_preempt(
        &mut new_plan,
        &mut new_current,
        PreemptKind::ThreatFlee { flee_target },
        None,
    );

    assert_eq!(outcome, PreemptOutcome::Preempted);
    assert_eq!(new_current.action, old_current.action);
    assert_eq!(new_current.ticks_remaining, old_current.ticks_remaining);
    assert_eq!(new_current.target_position, old_current.target_position);
    assert_eq!(new_current.target_entity, old_current.target_entity);
    assert_eq!(new_plan.current_step, old_plan.current_step);
}

#[test]
fn try_preempt_non_threat_matches_inline_body_resets_ticks_remaining() {
    // Critical 041 regression check: non-Threat preempts must reset
    // `ticks_remaining = 0`. Without it, every CriticalSafety /
    // CriticalHunger / Exhaustion preempt freezes the cat forever.
    let mut new_plan = fresh_plan();
    let mut new_current = fresh_current();
    let mut old_plan = fresh_plan();
    let mut old_current = fresh_current();

    // Inline body (verbatim from goap.rs:2363–2383, non-threat branch):
    old_plan.current_step = old_plan.steps.len();
    old_current.ticks_remaining = 0;

    // New API:
    let outcome = try_preempt(&mut new_plan, &mut new_current, PreemptKind::NonThreat, None);

    assert_eq!(outcome, PreemptOutcome::Preempted);
    assert_eq!(
        new_current.ticks_remaining, 0,
        "041 regression: non-Threat preempt must reset ticks_remaining"
    );
    assert_eq!(new_plan.current_step, old_plan.current_step);
    // action / target_position must be unchanged on non-threat preempt.
    assert_eq!(new_current.action, Action::Hunt);
    assert_eq!(new_current.target_position, None);
}

#[test]
fn try_preempt_threat_without_position_still_resets_ticks_remaining() {
    // The `urgent.threat_pos.is_none()` branch of the original inline
    // code reaches the unconditional reset at `goap.rs:2383` without
    // writing the flee target. Verify the API preserves this.
    let mut plan = fresh_plan();
    let mut current = fresh_current();
    current.action = Action::Hunt;

    let outcome = try_preempt(
        &mut plan,
        &mut current,
        PreemptKind::ThreatWithoutPosition,
        None,
    );

    assert_eq!(outcome, PreemptOutcome::Preempted);
    assert_eq!(current.ticks_remaining, 0);
    assert_eq!(plan.current_step, plan.steps.len());
    // No flee write happened.
    assert_eq!(current.action, Action::Hunt);
    assert_eq!(current.target_position, None);
}

// ---------------------------------------------------------------------------
// carry_target_forward — `goap.rs:2817–2820`
// ---------------------------------------------------------------------------

#[test]
fn carry_target_forward_matches_inline_body_when_unset() {
    let validity = target::InMemoryValidity::new();

    // Build two parallel step_state arrays.
    let mut new_steps = [
        StepExecutionState::default(),
        StepExecutionState::default(),
    ];
    let mut old_steps = [
        StepExecutionState::default(),
        StepExecutionState::default(),
    ];

    // Seed the prior step's target.
    let prior_target = Entity::from_raw_u32(7).unwrap();
    new_steps[0].target_entity = Some(prior_target);
    old_steps[0].target_entity = Some(prior_target);

    // Inline body (verbatim from goap.rs:2817–2820):
    let step_idx = 1;
    if old_steps[step_idx].target_entity.is_none() && step_idx > 0 {
        old_steps[step_idx].target_entity = old_steps[step_idx - 1].target_entity;
    }

    // New API:
    let result = carry_target_forward(&mut new_steps, step_idx, &validity, None);

    assert_eq!(new_steps[step_idx].target_entity, Some(prior_target));
    assert_eq!(
        new_steps[step_idx].target_entity,
        old_steps[step_idx].target_entity
    );
    assert_eq!(result, Some(prior_target));
}

#[test]
fn carry_target_forward_preserves_existing_target() {
    let validity = target::InMemoryValidity::new();
    let prior = Entity::from_raw_u32(1).unwrap();
    let already = Entity::from_raw_u32(99).unwrap();

    let mut steps = [
        StepExecutionState::default(),
        StepExecutionState::default(),
    ];
    steps[0].target_entity = Some(prior);
    steps[1].target_entity = Some(already);

    let result = carry_target_forward(&mut steps, 1, &validity, None);

    // Existing target_entity is not overwritten.
    assert_eq!(steps[1].target_entity, Some(already));
    assert_eq!(result, Some(already));
}

#[test]
fn carry_target_forward_at_step_zero_is_noop() {
    let validity = target::InMemoryValidity::new();
    let mut steps = [StepExecutionState::default()];

    let result = carry_target_forward(&mut steps, 0, &validity, None);

    assert_eq!(steps[0].target_entity, None);
    assert_eq!(result, None);
}

// ---------------------------------------------------------------------------
// validate_target — 074 implementation
// ---------------------------------------------------------------------------

#[test]
fn validate_target_alive_returns_ok() {
    let validity = target::InMemoryValidity::new();
    let result = validate_target(Entity::from_raw_u32(1).unwrap(), &validity);
    assert!(result.is_ok());
}

#[test]
fn validate_target_returns_dead_for_dead_entity() {
    let mut validity = target::InMemoryValidity::new();
    let e = Entity::from_raw_u32(1).unwrap();
    validity.mark(e, target::TargetInvalidReason::Dead);
    assert_eq!(
        validate_target(e, &validity),
        Err(target::TargetInvalidReason::Dead)
    );
}

#[test]
fn validate_target_returns_banished_for_banished_entity() {
    let mut validity = target::InMemoryValidity::new();
    let e = Entity::from_raw_u32(2).unwrap();
    validity.mark(e, target::TargetInvalidReason::Banished);
    assert_eq!(
        validate_target(e, &validity),
        Err(target::TargetInvalidReason::Banished)
    );
}

#[test]
fn validate_target_returns_incapacitated_for_incapacitated_entity() {
    let mut validity = target::InMemoryValidity::new();
    let e = Entity::from_raw_u32(3).unwrap();
    validity.mark(e, target::TargetInvalidReason::Incapacitated);
    assert_eq!(
        validate_target(e, &validity),
        Err(target::TargetInvalidReason::Incapacitated)
    );
}

#[test]
fn validate_target_returns_despawned_for_absent_entity() {
    let mut validity = target::InMemoryValidity::new();
    validity.absent_means_despawned = true;
    let e = Entity::from_raw_u32(4).unwrap();
    assert_eq!(
        validate_target(e, &validity),
        Err(target::TargetInvalidReason::Despawned)
    );
}

#[test]
fn carry_target_forward_drops_dead_prior_target() {
    // 074 — when the prior step's target is invalid, the carryover
    // copy is suppressed and the caller's failure path picks up the
    // `None` for replan.
    let mut validity = target::InMemoryValidity::new();
    let dead_target = Entity::from_raw_u32(7).unwrap();
    validity.mark(dead_target, target::TargetInvalidReason::Dead);

    let mut steps = [
        StepExecutionState::default(),
        StepExecutionState::default(),
    ];
    steps[0].target_entity = Some(dead_target);

    let result = carry_target_forward(&mut steps, 1, &validity, None);
    assert_eq!(
        result, None,
        "dead prior target must not propagate; caller replans"
    );
    assert_eq!(steps[1].target_entity, None);
}

// ---------------------------------------------------------------------------
// require_alive_filter — 074 implementation
// ---------------------------------------------------------------------------

#[test]
fn require_alive_filter_sets_require_target_alive() {
    let filter = require_alive_filter();
    assert!(filter.required.is_empty());
    assert!(filter.forbidden.is_empty());
    assert!(filter.required_stance.is_none());
    assert!(
        filter.require_target_alive,
        "074 — require_alive_filter() must set require_target_alive"
    );
}

// ---------------------------------------------------------------------------
// record_disposition_switch — `disposition.rs:1073` (legacy switch site)
// ---------------------------------------------------------------------------

#[test]
fn record_disposition_switch_writes_started_tick() {
    let p = test_personality();
    let mut disp = Disposition::new(DispositionKind::Hunting, crate::ai::Action::Hunt, 0, &p);
    assert_eq!(disp.disposition_started_tick, 0);

    record_disposition_switch(&mut disp, DispositionKind::Hunting, 12_345);
    assert_eq!(disp.disposition_started_tick, 12_345);
}

#[test]
fn record_disposition_switch_overwrites_prior_tick() {
    let p = test_personality();
    let mut disp = Disposition::new(DispositionKind::Hunting, crate::ai::Action::Hunt, 0, &p);
    record_disposition_switch(&mut disp, DispositionKind::Hunting, 100);
    record_disposition_switch(&mut disp, DispositionKind::Foraging, 250);
    assert_eq!(disp.disposition_started_tick, 250);
}

// ---------------------------------------------------------------------------
// Resource reservation — ticket 080
// ---------------------------------------------------------------------------

#[test]
fn require_unreserved_filter_sets_flag() {
    let filter = super::require_unreserved_filter();
    assert!(filter.require_unreserved);
    // The filter sets only the unreserved flag — no marker requirements
    // ride alongside (the gate is candidate-target-side, not cat-side).
    assert!(filter.required.is_empty());
    assert!(filter.forbidden.is_empty());
    assert!(filter.required_stance.is_none());
}

#[test]
fn reserved_component_records_owner_and_expiry() {
    use crate::components::reserved::Reserved;
    let owner = Entity::from_raw_u32(7).unwrap();
    let r = Reserved::new(owner, 1000, 600);
    assert_eq!(r.owner, owner);
    assert_eq!(r.expires_tick, 1600);
    assert!(!r.is_expired(1599));
    assert!(r.is_expired(1600));
    assert!(r.is_expired(2000));
    assert!(r.is_owned_by(owner));
    assert!(!r.is_owned_by(Entity::from_raw_u32(8).unwrap()));
}

#[test]
fn reserved_saturates_at_u64_max() {
    // Sanity: a u64::MAX tick + ttl shouldn't panic; saturating_add
    // pins the expiry at u64::MAX, which `is_expired` evaluates as
    // false until the world-clock crosses it (effectively never).
    use crate::components::reserved::Reserved;
    let owner = Entity::from_raw_u32(1).unwrap();
    let r = Reserved::new(owner, u64::MAX, 600);
    assert_eq!(r.expires_tick, u64::MAX);
    assert!(!r.is_expired(u64::MAX - 1));
}

#[test]
fn reserve_target_writes_component() {
    let mut world = World::new();
    let target = world.spawn_empty().id();
    let owner = world.spawn_empty().id();

    let mut commands_queue = bevy_ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut commands_queue, &world);
        super::reserve_target(&mut commands, target, owner, 100, 600);
    }
    commands_queue.apply(&mut world);

    let r = world
        .entity(target)
        .get::<crate::components::reserved::Reserved>()
        .expect("Reserved should be inserted");
    assert_eq!(r.owner, owner);
    assert_eq!(r.expires_tick, 700);
}

#[test]
fn release_target_removes_component() {
    let mut world = World::new();
    let owner = world.spawn_empty().id();
    let target = world
        .spawn(crate::components::reserved::Reserved::new(owner, 100, 600))
        .id();
    assert!(world
        .entity(target)
        .contains::<crate::components::reserved::Reserved>());

    let mut commands_queue = bevy_ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut commands_queue, &world);
        super::release_target(&mut commands, target);
    }
    commands_queue.apply(&mut world);

    assert!(!world
        .entity(target)
        .contains::<crate::components::reserved::Reserved>());
}

#[test]
fn release_target_idempotent_on_unreserved_entity() {
    // Releasing a never-reserved entity must not panic. `Commands::remove`
    // on a missing component is documented as a no-op.
    let mut world = World::new();
    let target = world.spawn_empty().id();

    let mut commands_queue = bevy_ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut commands_queue, &world);
        super::release_target(&mut commands, target);
    }
    commands_queue.apply(&mut world);
    assert!(!world
        .entity(target)
        .contains::<crate::components::reserved::Reserved>());
}

#[test]
fn require_unreserved_filter_gates_non_owner_to_zero() {
    // Per-candidate gate semantics: with `require_unreserved`, an
    // `is_reserved_by_other` closure that returns true for a candidate
    // drops it to score 0.0; the filter is a hard pass-through.
    use crate::ai::composition::Composition;
    use crate::ai::considerations::{Consideration, ScalarConsideration};
    use crate::ai::curves::Curve;
    use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
    use crate::ai::target_dse::{
        evaluate_target_taking_with_reservations, TargetAggregation, TargetTakingDse,
    };
    use crate::components::physical::Position;

    fn noop_intention(_: Entity) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Idle,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }

    fn noop_query(_: Entity) -> &'static str {
        "doc"
    }

    let dse = TargetTakingDse {
        id: DseId("test_unreserved"),
        candidate_query: noop_query,
        per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
            "target_quality",
            Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            },
        ))],
        composition: Composition::weighted_sum(vec![1.0]),
        aggregation: TargetAggregation::Best,
        intention: noop_intention,
        required_stance: None,
        eligibility: super::require_unreserved_filter(),
    };

    let cat = Entity::from_raw_u32(1).unwrap();
    let owner_target = Entity::from_raw_u32(10).unwrap();
    let stranger_target = Entity::from_raw_u32(11).unwrap();

    let has_marker = |_: &str, _: Entity| false;
    let entity_pos = |_: Entity| -> Option<Position> { None };
    let anchor_pos =
        |_: crate::ai::considerations::LandmarkAnchor| -> Option<Position> { None };
    let ctx = EvalCtx {
        cat,
        tick: 0,
        entity_position: &entity_pos,
        anchor_position: &anchor_pos,
        has_marker: &has_marker,
        self_position: Position::new(0, 0),
        target: None,
        target_position: None,
        target_alive: None,
    };
    let fetch_self = |_: &str, _: Entity| 0.0;
    // owner_target rates higher quality; without the gate it would win.
    let fetch_target = |_: &str, _: Entity, t: Entity| -> f32 {
        if t == owner_target {
            0.9
        } else {
            0.4
        }
    };
    // owner_target is reserved by someone other than `cat`; the gate
    // must drop it to 0.0 so stranger_target wins.
    let is_reserved_by_other = |target: Entity| -> bool { target == owner_target };

    let scored = evaluate_target_taking_with_reservations(
        &dse,
        cat,
        &[owner_target, stranger_target],
        &[Position::new(1, 0), Position::new(2, 0)],
        &ctx,
        &fetch_self,
        &fetch_target,
        Some(&is_reserved_by_other),
        None,
    );
    assert_eq!(
        scored.winning_target,
        Some(stranger_target),
        "reservation gate must drop owner_target so stranger wins"
    );
    // owner_target row is preserved at score 0.0 (gate writes 0.0,
    // doesn't drop the row).
    let owner_score = scored
        .per_target
        .iter()
        .find(|(e, _)| *e == owner_target)
        .map(|(_, s)| *s)
        .unwrap();
    assert_eq!(owner_score, 0.0, "gated candidate must score 0.0");
}

#[test]
fn require_unreserved_filter_passes_owner() {
    // Per-candidate gate semantics, owner case: when the scoring cat
    // *is* the owner of the reservation, the gate must NOT fire — the
    // owner continues to score the candidate normally.
    use crate::ai::composition::Composition;
    use crate::ai::considerations::{Consideration, ScalarConsideration};
    use crate::ai::curves::Curve;
    use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
    use crate::ai::target_dse::{
        evaluate_target_taking_with_reservations, TargetAggregation, TargetTakingDse,
    };
    use crate::components::physical::Position;

    fn noop_intention(_: Entity) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Idle,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn noop_query(_: Entity) -> &'static str {
        "doc"
    }

    let dse = TargetTakingDse {
        id: DseId("test_unreserved_owner"),
        candidate_query: noop_query,
        per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
            "target_quality",
            Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            },
        ))],
        composition: Composition::weighted_sum(vec![1.0]),
        aggregation: TargetAggregation::Best,
        intention: noop_intention,
        required_stance: None,
        eligibility: super::require_unreserved_filter(),
    };

    let cat = Entity::from_raw_u32(1).unwrap();
    let target = Entity::from_raw_u32(10).unwrap();

    let has_marker = |_: &str, _: Entity| false;
    let entity_pos = |_: Entity| -> Option<Position> { None };
    let anchor_pos =
        |_: crate::ai::considerations::LandmarkAnchor| -> Option<Position> { None };
    let ctx = EvalCtx {
        cat,
        tick: 0,
        entity_position: &entity_pos,
        anchor_position: &anchor_pos,
        has_marker: &has_marker,
        self_position: Position::new(0, 0),
        target: None,
        target_position: None,
        target_alive: None,
    };
    let fetch_self = |_: &str, _: Entity| 0.0;
    let fetch_target = |_: &str, _: Entity, _: Entity| 0.7;
    // Owner case — `is_reserved_by_other` returns false because the
    // caller resolves the (cat, target) pair against the snapshot and
    // `Reserved.owner == cat`.
    let is_reserved_by_other = |_target: Entity| -> bool { false };

    let scored = evaluate_target_taking_with_reservations(
        &dse,
        cat,
        &[target],
        &[Position::new(1, 0)],
        &ctx,
        &fetch_self,
        &fetch_target,
        Some(&is_reserved_by_other),
        None,
    );
    assert_eq!(scored.winning_target, Some(target));
    assert!((scored.aggregated_score - 0.7).abs() < 1e-5);
}

#[test]
fn require_unreserved_filter_inactive_when_dse_opts_out() {
    // DSEs without `require_unreserved` (`socialize_target`,
    // `groom_other_target`, etc.) must NOT consult the closure even
    // when the caller passes one — contention is OK by design for those
    // DSEs.
    use crate::ai::composition::Composition;
    use crate::ai::considerations::{Consideration, ScalarConsideration};
    use crate::ai::curves::Curve;
    use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
    use crate::ai::target_dse::{
        evaluate_target_taking_with_reservations, TargetAggregation, TargetTakingDse,
    };
    use crate::components::physical::Position;

    fn noop_intention(_: Entity) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Idle,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn noop_query(_: Entity) -> &'static str {
        "doc"
    }

    // Note `eligibility: Default::default()` — no require_unreserved.
    let dse = TargetTakingDse {
        id: DseId("test_no_unreserved"),
        candidate_query: noop_query,
        per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
            "target_quality",
            Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            },
        ))],
        composition: Composition::weighted_sum(vec![1.0]),
        aggregation: TargetAggregation::Best,
        intention: noop_intention,
        required_stance: None,
        eligibility: Default::default(),
    };

    let cat = Entity::from_raw_u32(1).unwrap();
    let target = Entity::from_raw_u32(10).unwrap();

    let has_marker = |_: &str, _: Entity| false;
    let entity_pos = |_: Entity| -> Option<Position> { None };
    let anchor_pos =
        |_: crate::ai::considerations::LandmarkAnchor| -> Option<Position> { None };
    let ctx = EvalCtx {
        cat,
        tick: 0,
        entity_position: &entity_pos,
        anchor_position: &anchor_pos,
        has_marker: &has_marker,
        self_position: Position::new(0, 0),
        target: None,
        target_position: None,
        target_alive: None,
    };
    let fetch_self = |_: &str, _: Entity| 0.0;
    let fetch_target = |_: &str, _: Entity, _: Entity| 0.7;
    // Closure says target is reserved — but the DSE doesn't opt in,
    // so the gate stays inactive. Score must reflect the underlying
    // consideration value.
    let is_reserved_by_other = |_target: Entity| -> bool { true };

    let scored = evaluate_target_taking_with_reservations(
        &dse,
        cat,
        &[target],
        &[Position::new(1, 0)],
        &ctx,
        &fetch_self,
        &fetch_target,
        Some(&is_reserved_by_other),
        None,
    );
    assert_eq!(scored.winning_target, Some(target));
    assert!((scored.aggregated_score - 0.7).abs() < 1e-5);
}

#[test]
fn expire_reservations_clears_past_due_markers() {
    // End-to-end: insert two `Reserved` markers — one with an
    // expires_tick in the past, one in the future. Run the
    // maintenance system; the past-due marker is removed, the
    // future-due marker survives.
    use bevy::prelude::*;
    use crate::components::reserved::Reserved;
    use crate::resources::time::TimeState;

    let mut app = App::new();
    app.insert_resource(TimeState {
        tick: 1000,
        ..Default::default()
    });
    let owner = app.world_mut().spawn_empty().id();
    let stale_target = app
        .world_mut()
        .spawn(Reserved::new(owner, 0, 100)) // expires_tick = 100
        .id();
    let fresh_target = app
        .world_mut()
        .spawn(Reserved::new(owner, 1000, 600)) // expires_tick = 1600
        .id();
    app.add_systems(Update, super::expire_reservations);
    app.update();

    assert!(
        !app.world()
            .entity(stale_target)
            .contains::<Reserved>(),
        "stale reservation must be cleared"
    );
    assert!(
        app.world()
            .entity(fresh_target)
            .contains::<Reserved>(),
        "future reservation must survive"
    );
}

#[test]
fn require_unreserved_fires_contention_hook() {
    // The `on_contention` hook must fire once per gated candidate so
    // resolvers can record `Feature::ReservationContended` without
    // scattering activation calls across every DSE.
    use crate::ai::composition::Composition;
    use crate::ai::considerations::{Consideration, ScalarConsideration};
    use crate::ai::curves::Curve;
    use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
    use crate::ai::target_dse::{
        evaluate_target_taking_with_reservations, TargetAggregation, TargetTakingDse,
    };
    use crate::components::physical::Position;

    fn noop_intention(_: Entity) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Idle,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn noop_query(_: Entity) -> &'static str {
        "doc"
    }

    let dse = TargetTakingDse {
        id: DseId("test_unreserved_hook"),
        candidate_query: noop_query,
        per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
            "target_quality",
            Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            },
        ))],
        composition: Composition::weighted_sum(vec![1.0]),
        aggregation: TargetAggregation::Best,
        intention: noop_intention,
        required_stance: None,
        eligibility: super::require_unreserved_filter(),
    };

    let cat = Entity::from_raw_u32(1).unwrap();
    let a = Entity::from_raw_u32(10).unwrap();
    let b = Entity::from_raw_u32(11).unwrap();

    let has_marker = |_: &str, _: Entity| false;
    let entity_pos = |_: Entity| -> Option<Position> { None };
    let anchor_pos =
        |_: crate::ai::considerations::LandmarkAnchor| -> Option<Position> { None };
    let ctx = EvalCtx {
        cat,
        tick: 0,
        entity_position: &entity_pos,
        anchor_position: &anchor_pos,
        has_marker: &has_marker,
        self_position: Position::new(0, 0),
        target: None,
        target_position: None,
        target_alive: None,
    };
    let fetch_self = |_: &str, _: Entity| 0.0;
    let fetch_target = |_: &str, _: Entity, _: Entity| 0.5;
    let is_reserved_by_other = |target: Entity| target == a; // only `a` is gated.

    let mut contended: Vec<Entity> = Vec::new();
    let mut on_contention =
        |target: Entity| contended.push(target);
    let _ = evaluate_target_taking_with_reservations(
        &dse,
        cat,
        &[a, b],
        &[Position::new(1, 0), Position::new(2, 0)],
        &ctx,
        &fetch_self,
        &fetch_target,
        Some(&is_reserved_by_other),
        Some(&mut on_contention),
    );
    assert_eq!(contended, vec![a]);
}
