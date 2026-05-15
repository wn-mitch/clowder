use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Wean`
///
/// 333 — vocabulary and method-flip landed with #333 (`rear_kitten`
/// is now `ApplicableWhen::Live`). **Dispatch is pending** — no
/// GoapActionKind / plan-template / dispatch arm wires the cat to
/// this resolver yet, because the HTN substrate (`HeldGoalStack`)
/// doesn't override `chosen_action` in the L2 evaluator. The
/// follow-on dispatch ticket (named in #333's landing Log) wires
/// DSE / GoapActionKind / resolver call site so this resolver
/// actually fires.
///
/// **Real-world effect** — when dispatch lands, advances a target
/// kitten's [`KittenDependency`](crate::components::KittenDependency)
/// past the `wean_threshold` maturity floor (clamped if maturity
/// is already higher), suppressing the mother-feeding pathway and
/// unlocking the next rearing sub-goal (Teach). Threshold constants
/// settled at dispatch time.
///
/// **Plan-level preconditions** — emitted under a kitten-target
/// predicate plus a maturity-floor check on the target's
/// `KittenDependency.maturity`.
///
/// **Runtime preconditions** — re-checks the kitten target still
/// carries `KittenDependency` and that the cat is the target's
/// recorded mother; returns `unwitnessed(Fail)` while dispatch is
/// unwired so accidental invocation is observable.
///
/// **Witness** — `StepOutcome<Option<bevy_ecs::entity::Entity>>`.
/// The witness payload is the kitten Entity whose maturity advanced
/// this call. The witness gates `Feature::KittenWeaned` emission via
/// `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::KittenWeaned`
/// (Positive) to `record_if_witnessed`. Ships
/// `expected_to_fire_per_soak() => false` until the dispatch
/// follow-on lands.
pub fn resolve_wean() -> StepOutcome<Option<bevy_ecs::entity::Entity>> {
    StepOutcome::unwitnessed(StepResult::Fail(
        "Wean dispatch wiring (DSE / GoapActionKind / plan template) pending — see follow-on ticket named in #333's landing Log".into(),
    ))
}
