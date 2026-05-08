//! 176 Drop resolver — `Action::Drop` / `DispositionKind::Discarding`.
//!
//! Releases one carried item from a cat's inventory onto the ground at
//! the cat's current position. The dropped item becomes a real `Item`
//! entity with `ItemLocation::OnGround`; another cat can later plan
//! `Action::PickUp` to retrieve it.

use bevy_ecs::prelude::*;

use crate::components::item_transfer::transfer_item_inventory_to_ground;
use crate::components::magic::Inventory;
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
/// `ItemLocation::OnGround` and removes the chosen slot from the cat's
/// `Inventory`. The drop is instant on entry; if the cat has nothing
/// to drop the step Fails.
///
/// **Plan-level preconditions** — emitted with no zone gate by
/// `src/ai/planner/actions.rs::discarding_actions` (terminal disposal),
/// or as a means-to-end prefix in pickup-class plans (ticket 231). The
/// Discarding disposition is at-position, no travel.
///
/// **Runtime preconditions** — at least one slot must be present in
/// `inventory`. Empty inventories cause a `Fail`; any slot kind is
/// droppable (the unified-pool `Inventory` makes herbs and items
/// indistinguishable for capacity purposes).
///
/// **Witness** — `StepOutcome<Option<DropOutcome>>`. `Some(outcome)`
/// on `StepResult::Advance` carries the spawned ground-item entity.
/// `None` on `Fail` (empty inventory).
///
/// **Feature emission** — caller passes `Feature::ItemDropped`
/// (Neutral) to `record_if_witnessed`.
pub fn resolve_drop_item(
    inventory: &mut Inventory,
    cat_pos: Position,
    commands: &mut Commands,
) -> StepOutcome<Option<DropOutcome>> {
    if inventory.slots.is_empty() {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "drop: empty inventory".to_string(),
        ));
    }
    let slot_idx = 0;

    match transfer_item_inventory_to_ground(inventory, slot_idx, cat_pos, commands) {
        Ok(item_entity) => {
            StepOutcome::witnessed_with(StepResult::Advance, DropOutcome { item_entity })
        }
        // The ground primitive cannot fail on capacity; surface as
        // Fail so the caller sees a concrete reason if it ever does.
        Err(_) => StepOutcome::unwitnessed(StepResult::Fail(
            "drop: transfer-to-ground primitive refused".to_string(),
        )),
    }
}
