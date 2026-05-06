//! 176 Drop resolver — `Action::Drop` / `DispositionKind::Discarding`.
//!
//! Releases one carried item from a cat's inventory onto the ground at
//! the cat's current position. The dropped item becomes a real `Item`
//! entity with `ItemLocation::OnGround`; another cat can later plan
//! `Action::PickUp` to retrieve it.

use bevy_ecs::prelude::*;

use crate::components::item_transfer::transfer_item_inventory_to_ground;
use crate::components::magic::{Inventory, ItemSlot};
use crate::components::physical::Position;
use crate::steps::{StepOutcome, StepResult};

/// Witness emitted on a successful drop. Carries the spawned ground-
/// item entity so the caller can record `Feature::ItemDropped` and
/// thread the entity into any focal-trace observability surface.
#[derive(Debug, Clone, Copy)]
pub struct DropOutcome {
    pub item_entity: Entity,
}

/// # GOAP step resolver: `DropItem`
///
/// **Real-world effect** — spawns one `Item` entity at `cat_pos` with
/// `ItemLocation::OnGround` and removes the corresponding slot from
/// the cat's `Inventory`. The drop is instant on entry; if the cat
/// has no `ItemSlot::Item(...)` slot to drop the step Fails.
///
/// **Plan-level preconditions** — emitted with no zone gate by
/// `src/ai/planner/actions.rs::discarding_actions`. The Discarding
/// disposition is at-position, no travel.
///
/// **Runtime preconditions** — at least one `ItemSlot::Item(...)`
/// must be present in `inventory`. Herb-only inventories cause a
/// `Fail` so the cat re-plans (the disposal DSE shouldn't have
/// elected this branch in the first place — herbs route through the
/// herbcraft disposal pathways).
///
/// **Witness** — `StepOutcome<Option<DropOutcome>>`. `Some(outcome)`
/// on `StepResult::Advance` carries the spawned ground-item entity.
/// `None` on `Fail` (no item in inventory).
///
/// **Feature emission** — caller passes `Feature::ItemDropped`
/// (Neutral) to `record_if_witnessed`.
pub fn resolve_drop_item(
    inventory: &mut Inventory,
    cat_pos: Position,
    commands: &mut Commands,
) -> StepOutcome<Option<DropOutcome>> {
    let Some(slot_idx) = inventory
        .slots
        .iter()
        .position(|s| matches!(s, ItemSlot::Item(_, _)))
    else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "drop: no item-slot in inventory".to_string(),
        ));
    };

    match transfer_item_inventory_to_ground(inventory, slot_idx, cat_pos, commands) {
        Ok(item_entity) => StepOutcome::witnessed_with(
            StepResult::Advance,
            DropOutcome { item_entity },
        ),
        // The ground primitive cannot fail on capacity; surface as
        // Fail so the caller sees a concrete reason if it ever does.
        Err(_) => StepOutcome::unwitnessed(StepResult::Fail(
            "drop: transfer-to-ground primitive refused".to_string(),
        )),
    }
}
