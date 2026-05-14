use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Wean`
///
/// 322 — dormant stub. Ticket #333 (kitten-rearing action vocabulary)
/// wires this resolver as the weaning sub-goal of the `rear_kitten`
/// method, keyed to `KittenDependency` on the kitten Entity. Until
/// then no Live HTN method emits `Action::Wean`, so this resolver is
/// never invoked at runtime; calling it returns `StepResult::Fail`
/// so accidental dispatch is observable.
///
/// **Real-world effect** — none today. When #333 lands, this resolver
/// will advance the kitten's `KittenDependency.stage` past
/// `Nursing`, suppressing the mother-feeding pathway and unlocking
/// the next rearing sub-goal (Teach).
///
/// **Plan-level preconditions** — none today. When #333 lands the
/// authoring chain will emit this step under a kitten-target
/// predicate (likely `StatePredicate::KittenInRange(target)`) along
/// with a maturity threshold on `KittenDependency`.
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable —
/// Feature emission is a compile-time error. The witness type flips
/// to `Option<Entity>` (the kitten Entity that progressed) when #333
/// authors the real resolver.
///
/// **Feature emission** — none today. When #333 lands, the real
/// resolver will pass a new `Feature::KittenWeaned` (Positive) to
/// `record_if_witnessed` at the witness site.
pub fn resolve_wean() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #333 (kitten-rearing action vocabulary) not yet landed".into(),
    ))
}
