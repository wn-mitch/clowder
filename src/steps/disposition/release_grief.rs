use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `ReleaseGrief`
///
/// 332 — terminal sub-goal of the `mourn_at_grave` HTN method. The
/// vocabulary, substrate (`Mourning` Component), and method-flip
/// landed with #332. **Dispatch is pending** — no GoapActionKind /
/// plan-template / dispatch-arm wires the cat to this resolver yet,
/// because the HTN substrate (`HeldGoalStack`) doesn't override
/// `chosen_action` in the L2 evaluator. The follow-on dispatch
/// ticket (named in #332's landing Log) wires DSE / GoapActionKind /
/// resolver call site so this resolver actually fires.
///
/// **Real-world effect** — when dispatch lands, removes the cat's
/// [`Mourning`](crate::components::Mourning) Component, signalling
/// the grief arc is complete. The `mourn_at_grave` method's
/// `MethodFailure::Backtrack` walks naturally on completion (frame
/// pops in `resolve_goap_plans`'s lifecycle).
///
/// **Plan-level preconditions** — the dispatch ticket emits this
/// step under `StatePredicate::HasMarker(Mourning::KEY)` so the plan
/// template only runs while grief is active.
///
/// **Runtime preconditions** — re-checks `Mourning` presence on the
/// cat (the planner's marker check can drift if a sibling system
/// retired the marker between plan and execution); returns
/// `unwitnessed(Advance)` if the marker is gone, `witnessed(Advance)`
/// after removing it.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the resolver
/// removed a `Mourning` Component this call. The witness gates
/// `Feature::GriefReleased` emission via `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::GriefReleased`
/// (Positive) to `record_if_witnessed`. Ships
/// `expected_to_fire_per_soak() => false` until the dispatch
/// follow-on lands and the §7.7.b grief-event-emission debt is
/// cleared.
pub fn resolve_release_grief() -> StepOutcome<bool> {
    StepOutcome::unwitnessed(StepResult::Fail(
        "ReleaseGrief dispatch wiring (DSE / GoapActionKind / plan template) pending — see follow-on ticket named in #332's landing Log".into(),
    ))
}
