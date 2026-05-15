use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Release`
///
/// 333 — vocabulary and method-flip landed with #333 (`rear_kitten`
/// is now `ApplicableWhen::Live`). **Dispatch is pending** — no
/// GoapActionKind / plan-template / dispatch arm wires the cat to
/// this resolver yet (HTN substrate doesn't override `chosen_action`
/// in the L2 evaluator). The follow-on dispatch ticket (named in
/// #333's landing Log) wires DSE / GoapActionKind / resolver call
/// site so this resolver actually fires.
///
/// **Real-world effect** — when dispatch lands, retires the mother's
/// `rear_kitten` method frame (the parent
/// [`HeldGoalStack`](crate::components::HeldGoalStack) walks the
/// abandon path) and removes the kitten target's
/// [`KittenDependency`](crate::components::KittenDependency)
/// Component, leaving the kitten as a fully independent colony
/// member. Distinct from `ReleaseGrief` (which retires a
/// `mourn_at_grave` arc — different real-world effect).
///
/// **Plan-level preconditions** — emitted under a maturity-threshold
/// check on the kitten target's `KittenDependency.maturity`
/// (post-Wean and post-Teach milestones cleared).
///
/// **Runtime preconditions** — re-checks the kitten target still
/// carries `KittenDependency` and that the cat is the recorded
/// mother; returns `unwitnessed(Fail)` while dispatch is unwired so
/// accidental invocation is observable.
///
/// **Witness** — `StepOutcome<Option<bevy_ecs::entity::Entity>>`.
/// The witness payload is the kitten Entity that was released this
/// call. The witness gates `Feature::KittenReleased` emission via
/// `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::KittenReleased`
/// (Positive) to `record_if_witnessed`. Ships
/// `expected_to_fire_per_soak() => false` until the dispatch
/// follow-on lands.
pub fn resolve_release() -> StepOutcome<Option<bevy_ecs::entity::Entity>> {
    StepOutcome::unwitnessed(StepResult::Fail(
        "Release dispatch wiring (DSE / GoapActionKind / plan template) pending — see follow-on ticket named in #333's landing Log".into(),
    ))
}
