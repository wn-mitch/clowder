use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::item_transfer::{
    transfer_item_stores_to_inventory, TransferError,
};
use crate::components::items::Item;
use crate::components::magic::Inventory;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `RetrieveFoodForKitten`
///
/// **Real-world effect** — transfers one food item (raw OR
/// cooked) from the target Stores building into the actor's
/// `Inventory`. Sibling to
/// `resolve_retrieve_raw_food_from_stores` but without the
/// `!cooked` filter — Caretake accepts either form.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(Stores)` with a follow-on
/// `SetCarrying(Carrying::RawFood)` in
/// `src/ai/planner/actions.rs::caretaking_actions` (§Phase 4c.4).
/// Without this step preceding `FeedKitten` in the Caretake plan,
/// `take_food()` in `resolve_feed_kitten` silently returned
/// `None` — the original silent-advance bug this audit is built
/// around.
///
/// **Runtime preconditions** — waits `ticks >= 5`. Requires
/// `target_entity` to resolve to a `StoredItems`, at least one
/// `item.kind.is_food()` item to be present, and free inventory
/// capacity. Ticket 175 routes through
/// `components::item_transfer::transfer_item_stores_to_inventory`;
/// on capacity miss the step returns
/// `unwitnessed(Fail("inventory full"))` rather than silently
/// destroying the item entity. On no-target / Stores-not-found /
/// no-matching-item: returns `unwitnessed(Advance)`.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff an item was
/// actually transferred from Stores to inventory this call.
///
/// **Feature emission** — caller passes `Feature::ItemRetrieved`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_retrieve_any_food_from_stores(
    ticks: u64,
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
        .find(|&e| items_query.get(e).is_ok_and(|item| item.kind.is_food()));
    let Some(item_entity) = target_item else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let Ok(item) = items_query.get(item_entity) else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    match transfer_item_stores_to_inventory(
        &mut stored,
        item_entity,
        item.kind,
        item.modifiers,
        inventory,
        commands,
    ) {
        Ok(()) => StepOutcome::witnessed(StepResult::Advance),
        Err(TransferError::DestinationFull) => {
            StepOutcome::unwitnessed(StepResult::Fail("inventory full".into()))
        }
    }
}
