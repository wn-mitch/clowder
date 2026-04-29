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
        100,
        &test_personality(),
        sample_steps(),
        None,
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
    record_step_failure(&mut new_plan, action, PlanFailureReason::Other, None, None);

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
    let validity = target::TargetValidityQuery;

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
    let validity = target::TargetValidityQuery;
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
    let validity = target::TargetValidityQuery;
    let mut steps = [StepExecutionState::default()];

    let result = carry_target_forward(&mut steps, 0, &validity, None);

    assert_eq!(steps[0].target_entity, None);
    assert_eq!(result, None);
}

// ---------------------------------------------------------------------------
// validate_target — stub (074 implements)
// ---------------------------------------------------------------------------

#[test]
fn validate_target_stub_always_ok() {
    let validity = target::TargetValidityQuery;
    let result = validate_target(Entity::from_raw_u32(1).unwrap(), &validity);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// require_alive_filter — stub (074 implements)
// ---------------------------------------------------------------------------

#[test]
fn require_alive_filter_stub_is_empty() {
    let filter = require_alive_filter();
    assert!(filter.required.is_empty());
    assert!(filter.forbidden.is_empty());
    assert!(filter.required_stance.is_none());
}

// ---------------------------------------------------------------------------
// record_disposition_switch — `disposition.rs:1073` (legacy switch site)
// ---------------------------------------------------------------------------

#[test]
fn record_disposition_switch_writes_started_tick() {
    let p = test_personality();
    let mut disp = Disposition::new(DispositionKind::Hunting, 0, &p);
    assert_eq!(disp.disposition_started_tick, 0);

    record_disposition_switch(&mut disp, DispositionKind::Hunting, 12_345);
    assert_eq!(disp.disposition_started_tick, 12_345);
}

#[test]
fn record_disposition_switch_overwrites_prior_tick() {
    let p = test_personality();
    let mut disp = Disposition::new(DispositionKind::Hunting, 0, &p);
    record_disposition_switch(&mut disp, DispositionKind::Hunting, 100);
    record_disposition_switch(&mut disp, DispositionKind::Foraging, 250);
    assert_eq!(disp.disposition_started_tick, 250);
}
