//! Plan-lifecycle operations: step-failure recording, abandonment,
//! preempt-cleanup. Bodies are lifted verbatim from `goap.rs` per the
//! ticket-072 mechanical-refactor contract; ticket 073 fleshes out the
//! `RecentTargetFailures` writes (audit gap #1 — cross-plan target
//! memory) at the same call sites.

use bevy_ecs::prelude::*;

use crate::ai::planner::GoapActionKind;
use crate::ai::{Action, CurrentAction};
use crate::components::physical::Position;
use crate::components::{
    AbandonReason, AbandonedPlanState, GoapPlan, PlanFailureReason, RecentTargetFailures,
};

// ---------------------------------------------------------------------------
// record_step_failure
// ---------------------------------------------------------------------------

/// Record a step's failure on the plan so replanning can exclude the
/// failing action. Lifted from `goap.rs:2451–2455` (072 mechanical
/// refactor); ticket 073 extends the body to also write into the
/// per-cat `RecentTargetFailures` map when the failed step has a
/// known `target_entity`.
///
/// **Real-world effect** — inserts `action` into `plan.failed_actions`
/// (consulted by the next replan call to exclude impossible actions).
/// When `target.is_some()` and `recent.is_some()`, also records the
/// `(action, target)` pair on `RecentTargetFailures` with the current
/// tick — this is the cross-plan memory that the
/// `target_recent_failure` Consideration consults on the six target-
/// taking DSEs.
///
/// `tick` is the current sim tick (caller-supplied to keep this
/// function pure).
///
/// `_reason` is unused today; ticket 075's `CommitmentTenure` Modifier
/// will branch on it (e.g., demote disposition tenure on
/// `PlanFailureReason::Resource` failures).
pub fn record_step_failure(
    plan: &mut GoapPlan,
    action: GoapActionKind,
    _reason: PlanFailureReason,
    target: Option<Entity>,
    recent: Option<&mut RecentTargetFailures>,
    tick: u64,
) {
    plan.failed_actions.insert(action);
    if let (Some(target), Some(recent)) = (target, recent) {
        recent.record(action, target, tick);
    }
}

// ---------------------------------------------------------------------------
// abandon_plan
// ---------------------------------------------------------------------------

/// Abandon a plan: reset the `CurrentAction` gate so next tick's
/// `evaluate_and_plan` re-evaluates, and signal the caller to remove
/// the `GoapPlan` component from the entity. Returns an
/// `AbandonedPlanState` snapshot.
///
/// Lifted from the two abandonment sites at `goap.rs:2574` and
/// `goap.rs:2593` — both inline bodies do `current.ticks_remaining = 0`
/// followed by `plans_to_remove.push(cat_entity)`. The narrative
/// `PlanEvent::Abandoned` emission and `commitment::record_drop` call
/// stay at the call site; they read context (focal capture, branch
/// classification) the substrate doesn't own.
///
/// **Real-world effect** — writes `current.ticks_remaining = 0`. The
/// caller is expected to push `cat_entity` onto its
/// `plans_to_remove` collection.
///
/// **Ticket 073 extension** — when `action.is_some()`,
/// `target.is_some()` and `recent.is_some()`, also records the
/// `(action, target)` pair on `RecentTargetFailures`. This is the
/// cross-plan memory bridge that closes audit gap #1: a replan-cap or
/// no-plan-possible abandonment destroys the plan's `failed_actions`
/// set, but the failure memory survives on the cat for the cooldown
/// window so the next plan's target-picker penalizes the same blocker.
///
/// `action` is the planner action whose target is being recorded —
/// callers pass the current step's `action.kind`. `tick` is the
/// current sim tick.
pub fn abandon_plan(
    current: &mut CurrentAction,
    _plan: &mut GoapPlan,
    _reason: AbandonReason,
    action: Option<GoapActionKind>,
    target: Option<Entity>,
    recent: Option<&mut RecentTargetFailures>,
    tick: u64,
) -> AbandonedPlanState {
    current.ticks_remaining = 0;
    if let (Some(action), Some(target), Some(recent)) = (action, target, recent) {
        recent.record(action, target, tick);
    }
    AbandonedPlanState
}

// ---------------------------------------------------------------------------
// try_preempt
// ---------------------------------------------------------------------------

/// What kind of preempt the caller is attempting. The kind decides
/// whether `try_preempt` writes a flee target onto `CurrentAction`
/// (ThreatNearby with a known threat position) or simply marks the
/// plan exhausted (every other urgency kind — CriticalSafety,
/// CriticalHunger, Exhaustion).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreemptKind {
    /// ThreatNearby preempt with a resolved flee target. Caller
    /// computed the flee target from `pos`, `flee_distance`, and the
    /// map bounds; the substrate writes it onto `CurrentAction` so
    /// the cat flees next tick.
    ThreatFlee { flee_target: Position },
    /// ThreatNearby preempt without a flee target (no `threat_pos`
    /// resolved). Mirrors the existing inline behavior — the plan is
    /// marked exhausted but `CurrentAction.action` is left as is. The
    /// load-bearing `ticks_remaining = 0` reset still fires.
    ThreatWithoutPosition,
    /// CriticalSafety / CriticalHunger / Exhaustion. No flee target
    /// is written; only the plan-exhaustion + ticks_remaining reset
    /// fire.
    NonThreat,
}

/// Outcome of `try_preempt`. Always `Preempted` today — the caller
/// makes the suppress / proceed decision before invoking. 072 keeps
/// the enum so 073/075 can extend without re-shaping the API.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreemptOutcome {
    /// Plan was preempted; caller should narrate `PlanEvent::Abandoned`
    /// and push `cat_entity` onto its `plans_to_remove` collection.
    Preempted,
}

/// Apply the plan-state mutations of a preempt. This function owns
/// **the load-bearing `current.ticks_remaining = 0` reset that closes
/// ticket 041** — every preempt kind, unconditional. Without that
/// reset, every non-ThreatNearby urgency drops the `GoapPlan` but
/// leaves `ticks_remaining = u64::MAX` (set at plan creation), which
/// `evaluate_and_plan`'s `if current.ticks_remaining != 0 { continue }`
/// filter then silently skips forever.
///
/// Lifted from `goap.rs:2342–2401` — the ThreatNearby flee-target
/// write, the `plan.current_step = plan.steps.len()` exhaustion mark,
/// and the unconditional `ticks_remaining = 0` reset. The flee-target
/// computation (vector-away-from-threat math + clamp to map bounds)
/// stays at the call site because it depends on map dimensions and
/// `flee_distance`; the substrate only consumes the resolved
/// `Position`.
///
/// **Real-world effect** — writes `plan.current_step = plan.steps.len()`,
/// `current.ticks_remaining = 0`. For `ThreatFlee`, additionally
/// writes `current.action = Action::Flee`, `current.target_position`,
/// `current.target_entity = None`.
///
/// `_recent` is reserved for future hardening tickets. Ticket 073
/// doesn't write here because preempts are caused by the cat's own
/// state (urgency, hunger, threat), not by the held target's failure
/// to cooperate — there's no specific candidate to penalize.
pub fn try_preempt(
    plan: &mut GoapPlan,
    current: &mut CurrentAction,
    kind: PreemptKind,
    _recent: Option<&mut RecentTargetFailures>,
) -> PreemptOutcome {
    if let PreemptKind::ThreatFlee { flee_target } = kind {
        current.action = Action::Flee;
        current.ticks_remaining = 0;
        current.target_position = Some(flee_target);
        current.target_entity = None;
    }

    // Mark plan exhausted so it flows through the normal
    // completion path for cleanup.
    plan.current_step = plan.steps.len();
    // Reset the CurrentAction gate so next tick's
    // `evaluate_and_plan` actually re-evaluates.
    // The ThreatFlee branch above sets this to 0 alongside
    // `Action::Flee`; without the unconditional reset here, every
    // other urgency kind (CriticalSafety / CriticalHunger /
    // Exhaustion) drops the GoapPlan but leaves
    // `ticks_remaining = u64::MAX` (set at plan creation).
    // `evaluate_and_plan`'s `if current.ticks_remaining != 0
    // { continue }` filter then silently skips the cat forever.
    // Ticket 041: Mallow stuck with `action=Cook` at the kitchen for
    // 13,000+ ticks after a CriticalSafety preempt of a Crafting
    // plan. The reset moved into `plan_substrate::try_preempt` in
    // ticket 072 so the fix is API-owned rather than inlined at one
    // branch.
    current.ticks_remaining = 0;

    PreemptOutcome::Preempted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{AbandonReason, DispositionKind, Personality};
    use crate::ai::planner::PlannedStep;

    fn entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).unwrap()
    }

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

    fn fresh_plan() -> GoapPlan {
        GoapPlan::new(
            DispositionKind::Socializing,
            crate::ai::Action::Socialize,
            0,
            &test_personality(),
            vec![PlannedStep {
                action: GoapActionKind::SocializeWith,
                cost: 1,
            }],
        )
    }

    #[test]
    fn record_step_failure_writes_recent_when_target_known() {
        let mut plan = fresh_plan();
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        record_step_failure(
            &mut plan,
            GoapActionKind::SocializeWith,
            PlanFailureReason::Other,
            Some(target),
            Some(&mut recent),
            500,
        );
        assert!(plan.failed_actions.contains(&GoapActionKind::SocializeWith));
        assert_eq!(
            recent.last_failure_tick(GoapActionKind::SocializeWith, target),
            Some(500)
        );
    }

    #[test]
    fn record_step_failure_skips_recent_when_no_target() {
        let mut plan = fresh_plan();
        let mut recent = RecentTargetFailures::default();
        record_step_failure(
            &mut plan,
            GoapActionKind::TravelTo(crate::ai::planner::PlannerZone::Stores),
            PlanFailureReason::Other,
            None,
            Some(&mut recent),
            500,
        );
        assert!(plan.failed_actions.contains(&GoapActionKind::TravelTo(
            crate::ai::planner::PlannerZone::Stores
        )));
        assert!(recent.is_empty());
    }

    #[test]
    fn abandon_plan_writes_recent_when_action_and_target_known() {
        let mut current = CurrentAction::default();
        current.ticks_remaining = u64::MAX;
        let mut plan = fresh_plan();
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        let _ = abandon_plan(
            &mut current,
            &mut plan,
            AbandonReason::ReplanCap,
            Some(GoapActionKind::SocializeWith),
            Some(target),
            Some(&mut recent),
            900,
        );
        assert_eq!(current.ticks_remaining, 0);
        assert_eq!(
            recent.last_failure_tick(GoapActionKind::SocializeWith, target),
            Some(900)
        );
    }

    #[test]
    fn abandon_plan_resets_ticks_even_without_recent() {
        let mut current = CurrentAction::default();
        current.ticks_remaining = u64::MAX;
        let mut plan = fresh_plan();
        let _ = abandon_plan(
            &mut current,
            &mut plan,
            AbandonReason::NoPlanPossible,
            None,
            None,
            None,
            0,
        );
        assert_eq!(current.ticks_remaining, 0);
    }
}
