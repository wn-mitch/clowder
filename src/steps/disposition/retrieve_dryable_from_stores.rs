use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::item_transfer::{transfer_item_stores_to_inventory, TransferError};
use crate::components::items::{Item, ItemKind};
use crate::components::magic::Inventory;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `RetrieveDryable`
///
/// **Real-world effect** — transfers one `ItemKind::RawFish` or
/// `ItemKind::RawOrgan` item from the target Stores building into the
/// cat's `Inventory`. Paired with a subsequent `DryFood` step that
/// consumes the retrieved item onto a Drying Rack.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Stores)` with
/// a follow-on `SetCarrying(Carrying::RawFood)` effect in
/// `src/ai/planner/actions.rs::drying_food_actions`. Mirrors the
/// `RetrieveRawFood` step's contract but with a tighter item-kind
/// filter (drying recipes can't accept mammal/bird raw meat — the
/// load resolver would Fail).
///
/// **Runtime preconditions** — waits `ticks >= 5` (matches
/// `resolve_retrieve_raw_food_from_stores`). Requires `target_entity`
/// to resolve to a `StoredItems`, and for at least one stored item to
/// satisfy `is_dryable(item.kind) && !modifiers.cooked`. Inventory
/// must have a free slot — the typed transfer primitive
/// (`transfer_item_stores_to_inventory`) checks capacity before
/// removing from Stores so a full inventory never silently destroys a
/// real item. On no-target / no-matching-item / Stores-not-found:
/// returns `unwitnessed(Advance)` so the chain moves on (the
/// substrate said dryables were available but the cat arrived after
/// another cat claimed them).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff an item was actually
/// transferred from Stores to inventory this call.
///
/// **Feature emission** — caller passes `Feature::ItemRetrieved`
/// (Positive) to `record_if_witnessed`, same as the sibling
/// `RetrieveRawFood` retrieval. The downstream `DryFood` step emits
/// `Feature::FoodLoadedOnDryingRack` when the load actually fires.
pub fn resolve_retrieve_dryable_from_stores(
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
    let target_item = stored.items.iter().copied().find(|&e| {
        items_query.get(e).is_ok_and(|item| {
            !item.modifiers.cooked && matches!(item.kind, ItemKind::RawFish | ItemKind::RawOrgan)
        })
    });
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
