use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Craft`
///
/// 322 — dormant stub. Ticket #334 (stealth-cloak crafting recipe +
/// WearItem resolver) wires this resolver alongside the crafting
/// substrate (`docs/systems/crafting.md`). Until then no Live HTN
/// method emits `Action::Craft`, so this resolver is never invoked at
/// runtime; calling it returns `StepResult::Fail` so accidental
/// dispatch is observable.
///
/// **Real-world effect** — none today. When #334 lands, this resolver
/// will consume input items from `Inventory` per a registered
/// crafting recipe and produce the output item (e.g. StealthCloak
/// from raw materials), spawning the real `Item` entity per the
/// CLAUDE.md "items are real" pillar.
///
/// **Plan-level preconditions** — none today. When #334 lands the
/// authoring chain will emit this step under
/// `StatePredicate::CarryingIs(Carrying::CraftingMaterials)` and
/// `StatePredicate::AtWorkshop` (or whatever shape the crafting
/// substrate introduces).
///
/// **Runtime preconditions** — none today. This is a dormant stub per
/// `docs/systems/htn-methods.md` §G / Action-enum stubs. Calling it
/// returns `StepResult::Fail` with a blocker-named reason.
///
/// **Witness** — `StepOutcome<()>`. Witness-less; `()` does not
/// implement `Witnessed`, so `record_if_witnessed` is not callable —
/// Feature emission is a compile-time error. The witness type flips
/// to `Option<Entity>` (the crafted item) when #334 authors the real
/// resolver.
///
/// **Feature emission** — none today. When #334 lands, the real
/// resolver will pass a new `Feature::ItemCrafted` (Positive) to
/// `record_if_witnessed` at the witness site.
pub fn resolve_craft() -> StepOutcome<()> {
    StepOutcome::bare(StepResult::Fail(
        "ticket #334 (stealth-cloak crafting recipe + WearItem resolver) not yet landed".into(),
    ))
}
