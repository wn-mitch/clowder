use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `GriefSit`
///
/// 332 — vocabulary, substrate (`Mourning` Component), and method-flip
/// landed with #332. **Dispatch is pending** — no GoapActionKind /
/// plan-template / dispatch arm wires the cat to this resolver yet
/// (HTN substrate doesn't override `chosen_action` in the L2
/// evaluator). The follow-on dispatch ticket (named in #332's
/// landing Log) wires DSE / GoapActionKind / resolver call site so
/// this resolver actually fires.
///
/// **Real-world effect** — when dispatch lands, holds the cat in
/// their den for `grief_sit_duration_ticks`, advancing the
/// mourning-cycle counter on the cat's
/// [`Mourning`](crate::components::Mourning) Component while
/// suppressing higher-tier Maslow needs for the duration (mirrors
/// the way `Sleep`'s resolver handles in-place rest). Counter shape
/// settled at dispatch time.
///
/// **Plan-level preconditions** — emitted under a den-proximity
/// predicate plus `StatePredicate::HasMarker(Mourning::KEY)`.
///
/// **Runtime preconditions** — re-checks `Mourning` presence on the
/// cat; returns `unwitnessed(Fail)` while dispatch is unwired so
/// accidental invocation is observable.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the cat performed a
/// grief-sit tick this call (counter advanced).
///
/// **Feature emission** — caller passes `Feature::GriefProcessed`
/// (Positive) to `record_if_witnessed`. Ships
/// `expected_to_fire_per_soak() => false` until the dispatch
/// follow-on lands.
pub fn resolve_grief_sit() -> StepOutcome<bool> {
    StepOutcome::unwitnessed(StepResult::Fail(
        "GriefSit dispatch wiring (DSE / GoapActionKind / plan template) pending — see follow-on ticket named in #332's landing Log".into(),
    ))
}
