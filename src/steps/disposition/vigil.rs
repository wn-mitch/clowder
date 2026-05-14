use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Vigil`
///
/// 322 — dormant stub. Ticket #332 (grief-vigil action vocabulary)
/// wires this resolver alongside the Grave-target picker for the
/// `mourn_at_grave` method. Until then no Live HTN method emits
/// `Action::Vigil`, so this resolver is never invoked at runtime;
/// calling it returns `StepResult::Fail` so accidental dispatch is
/// observable.
///
/// **Real-world effect** — none today. When #332 lands, this resolver
/// will hold the cat in vigil at a Grave entity (sitting still,
/// optionally accruing a per-tick grief-decay tick), advancing the
/// mourning-cycle counter on a `Mourning` Component or similar.
///
/// **Plan-level preconditions** — none today. When #332 lands the
/// authoring chain will emit this step under a Grave-proximity
/// predicate (likely a new `StatePredicate::AtGrave(target)` keyed to
/// the Grave entity selected by the target-picker).
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable —
/// Feature emission is a compile-time error. The witness type flips
/// to `bool` (vigil tick performed) when #332 authors the real
/// resolver.
///
/// **Feature emission** — none today. When #332 lands, the real
/// resolver will pass a new `Feature::VigilHeld` (Positive) to
/// `record_if_witnessed` at the witness site.
pub fn resolve_vigil() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #332 (grief-vigil action vocabulary) not yet landed".into(),
    ))
}
