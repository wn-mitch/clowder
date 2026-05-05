use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::item_transfer::{
    transfer_item_stores_to_inventory, TransferError,
};
use crate::components::items::{Item, ItemKind};
use crate::components::magic::Inventory;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `RetrieveFromStores`
///
/// **Real-world effect** — transfers one item of a specific
/// `ItemKind` from a target Stores building into the actor's
/// `Inventory`.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(Stores)` by various task-chain builders that need a
/// specific kind (cooked food, herbs, etc).
///
/// **Runtime preconditions** — waits `ticks >= 5`. Requires
/// `target_entity` to resolve to a `StoredItems`, a matching
/// `ItemKind` to be present, and free inventory capacity.
/// Ticket 175 routes the transfer through
/// `components::item_transfer::transfer_item_stores_to_inventory`
/// to maintain the "items are real" invariant; on capacity miss
/// the step returns `unwitnessed(Fail("inventory full"))` rather
/// than silently destroying the item entity. On no-target /
/// Stores-not-found / no-matching-item: returns
/// `unwitnessed(Advance)` — the chain moves on (the substrate
/// said the item was available but the cat arrived after another
/// cat claimed it).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff an item was
/// actually transferred.
///
/// **Feature emission** — caller passes `Feature::ItemRetrieved`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_retrieve_from_stores(
    ticks: u64,
    kind: ItemKind,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    commands: &mut Commands,
) -> StepOutcome<bool> {
    if ticks < 5 {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }
    let Some(store_entity) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let Ok(mut stored) = stores_query.get_mut(store_entity) else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let target_item = stored
        .items
        .iter()
        .copied()
        .find(|&e| items_query.get(e).is_ok_and(|item| item.kind == kind));
    let Some(item_entity) = target_item else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let modifiers = items_query
        .get(item_entity)
        .map(|item| item.modifiers)
        .unwrap_or_default();
    match transfer_item_stores_to_inventory(
        &mut stored,
        item_entity,
        kind,
        modifiers,
        inventory,
        commands,
    ) {
        Ok(()) => StepOutcome::witnessed(StepResult::Advance),
        Err(TransferError::DestinationFull) => {
            StepOutcome::unwitnessed(StepResult::Fail("inventory full".into()))
        }
    }
}
