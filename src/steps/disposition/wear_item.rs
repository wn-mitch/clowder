use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `WearItem`
///
/// 322 — dormant stub. Ticket #334 (stealth-cloak crafting recipe +
/// WearItem resolver) wires this resolver alongside the slot-inventory
/// substrate and the StealthCloak recipe. Until then no Live HTN method
/// emits `Action::WearItem`, so this resolver is never invoked at
/// runtime; calling it returns `StepResult::Fail` so accidental
/// dispatch is observable rather than silent.
///
/// **Real-world effect** — none today. When #334 lands, this resolver
/// will move a worn-slot-eligible item from `Inventory` into the cat's
/// worn-slot Component, granting the item's passive effect (e.g.
/// stealth-cloak's stalk-success multiplier on the Hunt resolver per
/// CLAUDE.md "items are real" pillar — effects on resolvers keyed to
/// item identity, not abstract stat modifiers).
///
/// **Plan-level preconditions** — none today. When #334 lands the
/// authoring chain will emit this step under
/// `StatePredicate::CarryingIs(Carrying::WearableGear)` (or the
/// equivalent precondition the slot-inventory substrate introduces).
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason so the
/// failure surfaces in the L3 trace if dispatch ever reaches it.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable on
/// this shape — Feature emission from this resolver is a compile-time
/// error. The witness type flips to `bool` or `Option<Entity>` when
/// #334 authors the real resolver.
///
/// **Feature emission** — none today. When #334 lands, the real
/// resolver will pass a new `Feature::ItemWorn` (Positive) to
/// `record_if_witnessed` at the witness site.
pub fn resolve_wear_item() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #334 (stealth-cloak crafting recipe + WearItem resolver) not yet landed".into(),
    ))
}
