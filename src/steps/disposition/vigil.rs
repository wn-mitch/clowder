use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Vigil`
///
/// 332 — vocabulary, substrate (`Mourning` Component), and method-flip
/// landed with #332 (`mourn_at_grave` is now `ApplicableWhen::Live`).
/// **Dispatch is pending** — no GoapActionKind / plan-template /
/// dispatch arm wires the cat to this resolver yet, because the HTN
/// substrate (`HeldGoalStack`) doesn't override `chosen_action` in
/// the L2 evaluator. The follow-on dispatch ticket (named in #332's
/// landing Log) wires DSE / GoapActionKind / resolver call site so
/// this resolver actually fires.
///
/// **Real-world effect** — when dispatch lands, holds the cat at a
/// `Grave` entity for `vigil_duration_ticks` (sitting still, no
/// inventory mutation), advancing the mourning-cycle counter on the
/// cat's [`Mourning`](crate::components::Mourning) Component (the
/// counter shape is settled by the dispatch ticket alongside the
/// vigil-duration constant).
///
/// **Plan-level preconditions** — the dispatch ticket emits this
/// step under `StatePredicate::HasMarker(Mourning::KEY)` plus a
/// grave-proximity predicate. Until then, no plan template includes
/// `Vigil`, so the precondition contract is settled at dispatch time.
///
/// **Runtime preconditions** — re-checks that the cat still holds
/// `Mourning` (the planner's marker check can drift if a sibling
/// system retired the marker between plan and execution); returns
/// `unwitnessed(Fail)` when dispatch is unwired so accidental
/// invocation is observable rather than silently advancing.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the cat performed a
/// real vigil tick this call (counter advanced, real-world effect).
/// The witness gates `Feature::VigilHeld` emission via
/// `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::VigilHeld`
/// (Positive) to `record_if_witnessed`. Ships
/// `expected_to_fire_per_soak() => false` until the dispatch
/// follow-on lands and the §7.7.b grief-event-emission debt is
/// cleared.
pub fn resolve_vigil() -> StepOutcome<bool> {
    StepOutcome::unwitnessed(StepResult::Fail(
        "Vigil dispatch wiring (DSE / GoapActionKind / plan template) pending — see follow-on ticket named in #332's landing Log".into(),
    ))
}
