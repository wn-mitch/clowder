use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `PetitionCoordinator`
///
/// 322 — dormant stub. Ticket #334 wires this resolver as part of the
/// `acquire_stealth_via_commission` method (the commission flow needs
/// both the petition channel and the coordinator-side fulfillment
/// substrate). Until then no Live HTN method emits
/// `Action::PetitionCoordinator`, so this resolver is never invoked
/// at runtime; calling it returns `StepResult::Fail` so accidental
/// dispatch is observable.
///
/// **Real-world effect** — none today. When #334 lands, this resolver
/// will register a pending request from the petitioning cat to a
/// nearby coordinator (e.g. "please commission a stealth cloak for
/// me"), inserting a request entity or Component the coordinator's
/// later evaluator reads.
///
/// **Plan-level preconditions** — none today. When #334 lands the
/// authoring chain will emit this step under
/// `StatePredicate::NearCoordinator` (or the equivalent
/// social-coordination predicate that lands with the strategist-
/// coordinator substrate).
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable —
/// Feature emission is a compile-time error. The witness type flips
/// to `Option<Entity>` (the request entity) when #334 authors the
/// real resolver.
///
/// **Feature emission** — none today. When #334 lands, the real
/// resolver will pass a new `Feature::CoordinatorPetitioned`
/// (Positive) to `record_if_witnessed` at the witness site.
pub fn resolve_petition_coordinator() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #334 (stealth-cloak crafting recipe + WearItem resolver) not yet landed".into(),
    ))
}
