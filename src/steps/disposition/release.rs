use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Release`
///
/// 322 — dormant stub. Ticket #333 (kitten-rearing action vocabulary)
/// wires this resolver as the terminal sub-goal of the `rear_kitten`
/// method — the moment the mother releases the now-independent
/// kitten to the colony. Until then no Live HTN method emits
/// `Action::Release`, so this resolver is never invoked at runtime;
/// calling it returns `StepResult::Fail` so accidental dispatch is
/// observable.
///
/// **Real-world effect** — none today. When #333 lands, this resolver
/// will retire the mother's `rear_kitten` method frame (the parent
/// `HeldGoalStack` walks the abandon path) and clear the
/// kitten's `KittenDependency` Component, leaving the kitten as a
/// fully independent colony member.
///
/// **Plan-level preconditions** — none today. When #333 lands the
/// authoring chain will emit this step under a maturity-threshold
/// check on `KittenDependency` (post-Wean and post-Teach milestones
/// hit).
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable —
/// Feature emission is a compile-time error. The witness type flips
/// to `Option<Entity>` (the kitten Entity that was released) when
/// #333 authors the real resolver.
///
/// **Feature emission** — none today. When #333 lands, the real
/// resolver will pass a new `Feature::KittenReleased` (Positive) to
/// `record_if_witnessed` at the witness site.
pub fn resolve_release() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #333 (kitten-rearing action vocabulary) not yet landed".into(),
    ))
}
