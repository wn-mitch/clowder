use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `GriefSit`
///
/// 322 — dormant stub. Ticket #332 (grief-vigil action vocabulary)
/// wires this resolver as the "grieve-in-den" sub-goal of the
/// `mourn_at_grave` method. Until then no Live HTN method emits
/// `Action::GriefSit`, so this resolver is never invoked at runtime;
/// calling it returns `StepResult::Fail` so accidental dispatch is
/// observable.
///
/// **Real-world effect** — none today. When #332 lands, this resolver
/// will hold the cat in their den, advancing the mourning-cycle
/// counter while suppressing the higher-tier Maslow needs for one
/// tick (mirrors the way Sleep's resolver handles in-place rest).
///
/// **Plan-level preconditions** — none today. When #332 lands the
/// authoring chain will emit this step under a den-proximity
/// predicate alongside the `Mourning` Component check.
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable —
/// Feature emission is a compile-time error. The witness type flips
/// to `bool` (grief-sit tick performed) when #332 authors the real
/// resolver.
///
/// **Feature emission** — none today. When #332 lands, the real
/// resolver will pass a new `Feature::GriefProcessed` (Positive) to
/// `record_if_witnessed` at the witness site.
pub fn resolve_grief_sit() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #332 (grief-vigil action vocabulary) not yet landed".into(),
    ))
}
