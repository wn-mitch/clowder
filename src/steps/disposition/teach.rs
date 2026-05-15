use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Teach`
///
/// 333 — vocabulary and method-flip landed with #333 (`rear_kitten`
/// is now `ApplicableWhen::Live`). **Dispatch is pending** — no
/// GoapActionKind / plan-template / dispatch arm wires the cat to
/// this resolver yet (HTN substrate doesn't override `chosen_action`
/// in the L2 evaluator). The follow-on dispatch ticket (named in
/// #333's landing Log) wires DSE / GoapActionKind / resolver call
/// site so this resolver actually fires.
///
/// **Real-world effect** — when dispatch lands, demonstrates a skill
/// (forage / hunt-stalk / etc, picked from a curriculum table) to a
/// target kitten, advancing the target's
/// [`KittenDependency`](crate::components::KittenDependency)
/// `skills_learned` tally (a future field added alongside the
/// dispatch ticket) and seeding a memory record on the kitten side.
///
/// **Plan-level preconditions** — emitted under a kitten-target
/// predicate plus a "skill-not-yet-taught" check.
///
/// **Runtime preconditions** — re-checks the kitten target still
/// carries `KittenDependency`; returns `unwitnessed(Fail)` while
/// dispatch is unwired so accidental invocation is observable.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the cat performed a
/// teaching tick this call (skill demonstration recorded). The
/// witness gates `Feature::SkillTaught` emission via
/// `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::SkillTaught`
/// (Positive) to `record_if_witnessed`. Ships
/// `expected_to_fire_per_soak() => false` until the dispatch
/// follow-on lands.
pub fn resolve_teach() -> StepOutcome<bool> {
    StepOutcome::unwitnessed(StepResult::Fail(
        "Teach dispatch wiring (DSE / GoapActionKind / plan template) pending — see follow-on ticket named in #333's landing Log".into(),
    ))
}
