//! 176 Handoff resolver — `Action::Handoff` / `DispositionKind::Handing`.
//!
//! Transfers one item from the actor cat's inventory to a target
//! cat's inventory. Slot-to-slot — no entity churn since inventory
//! slots are value-typed.

use bevy_ecs::prelude::*;

use crate::components::item_transfer::transfer_item_inventory_to_inventory;
use crate::components::magic::{Inventory, ItemSlot};
use crate::steps::{StepOutcome, StepResult};

/// Witness emitted on a successful handoff. The caller applies any
/// downstream relationship / fondness changes in a post-loop pass
/// (similar to the Groom resolver pattern, since `&mut Inventory`
/// on the target conflicts with the actor's borrow in the cats
/// query — the actual transfer is done here, but per-cat side
/// effects on the recipient are deferred via the witness).
#[derive(Debug, Clone, Copy)]
pub struct HandoffOutcome {
    pub recipient: Entity,
}

/// # GOAP step resolver: `HandoffItem`
///
/// **Real-world effect** — moves one `ItemSlot::Item(...)` (or
/// `ItemSlot::Herb(...)`) from the actor's inventory to the target
/// cat's inventory. Both inventories are mutated in this resolver;
/// the caller threads them in via the cat-pair query split.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(SocialTarget)` by `handing_actions`. The L2 disposal DSE
/// picks the recipient cat and threads it as `target_entity`.
///
/// **Runtime preconditions** — `target_entity` must resolve to a cat
/// with `Inventory` having room. The actor must hold at least one
/// item-or-herb slot.
///
/// **Witness** — `StepOutcome<Option<HandoffOutcome>>`. `Some(outcome)`
/// on `Advance` carries the recipient entity. `None` on `Fail`.
///
/// **Feature emission** — caller passes `Feature::ItemHandedOff`
/// (Neutral) to `record_if_witnessed`.
pub fn resolve_handoff(
    actor_inventory: &mut Inventory,
    recipient: Entity,
    recipient_inventory: &mut Inventory,
) -> StepOutcome<Option<HandoffOutcome>> {
    let Some(slot_idx) = actor_inventory
        .slots
        .iter()
        .position(|s| matches!(s, ItemSlot::Item(_, _) | ItemSlot::Herb(_)))
    else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "handoff: no transferable slot in actor inventory".to_string(),
        ));
    };

    match transfer_item_inventory_to_inventory(actor_inventory, slot_idx, recipient_inventory) {
        Ok(()) => StepOutcome::witnessed_with(
            StepResult::Advance,
            HandoffOutcome { recipient },
        ),
        Err(_) => StepOutcome::unwitnessed(StepResult::Fail(
            "handoff: recipient inventory full".to_string(),
        )),
    }
}
