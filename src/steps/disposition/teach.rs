use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Teach`
///
/// 322 — dormant stub. Ticket #333 (kitten-rearing action vocabulary)
/// wires this resolver as the teaching sub-goal of the `rear_kitten`
/// method. Until then no Live HTN method emits `Action::Teach`, so
/// this resolver is never invoked at runtime; calling it returns
/// `StepResult::Fail` so accidental dispatch is observable.
///
/// **Real-world effect** — none today. When #333 lands, this resolver
/// will demonstrate a skill (e.g. forage, hunt-stalk) to the kitten
/// target, advancing the kitten's `KittenDependency.skills_learned`
/// tally and seeding a small Memory record on the kitten side.
///
/// **Plan-level preconditions** — none today. When #333 lands the
/// authoring chain will emit this step under a kitten-target
/// predicate plus a "skill-not-yet-taught" check.
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable —
/// Feature emission is a compile-time error. The witness type flips
/// to `bool` (teaching tick performed) when #333 authors the real
/// resolver.
///
/// **Feature emission** — none today. When #333 lands, the real
/// resolver will pass a new `Feature::SkillTaught` (Positive) to
/// `record_if_witnessed` at the witness site.
pub fn resolve_teach() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #333 (kitten-rearing action vocabulary) not yet landed".into(),
    ))
}
