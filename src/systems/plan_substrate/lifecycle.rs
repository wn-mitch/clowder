//! Plan-lifecycle operations: step-failure recording, abandonment,
//! preempt-cleanup. Bodies are lifted verbatim from `goap.rs` per the
//! ticket-072 mechanical-refactor contract.

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
/// failing action. Lifted from `goap.rs:2451–2455`. **072 stub:** the
/// `_reason`, `_target`, `_recent` parameters are reserved for ticket
/// 073 (per-target failure memory); today the body only mirrors the
/// existing `plan.failed_actions.insert(action)` behavior.
///
/// **Real-world effect** — inserts `action` into `plan.failed_actions`,
/// which is consulted by the next replan call to exclude impossible
/// actions.
pub fn record_step_failure(
    plan: &mut GoapPlan,
    action: GoapActionKind,
    _reason: PlanFailureReason,
    _target: Option<Entity>,
    _recent: Option<&mut RecentTargetFailures>,
) {
    plan.failed_actions.insert(action);
}

// ---------------------------------------------------------------------------
// abandon_plan
// ---------------------------------------------------------------------------

/// Abandon a plan: reset the `CurrentAction` gate so next tick's
/// `evaluate_and_plan` re-evaluates, and signal the caller to remove
/// the `GoapPlan` component from the entity. Returns an
/// `AbandonedPlanState` snapshot; today (072) the snapshot is empty,
/// 073 extends it with cross-plan target memory.
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
/// `plans_to_remove` collection (the substrate doesn't own that
/// list — it lives in `resolve_goap_plans`'s loop locals).
pub fn abandon_plan(
    current: &mut CurrentAction,
    _plan: &mut GoapPlan,
    _reason: AbandonReason,
    _recent: Option<&mut RecentTargetFailures>,
) -> AbandonedPlanState {
    current.ticks_remaining = 0;
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
