use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::item_transfer::{
    transfer_item_stores_to_inventory, TransferError,
};
use crate::components::items::Item;
use crate::components::magic::Inventory;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `RetrieveRawFood`
///
/// **Real-world effect** — transfers one raw (uncooked) food item
/// from the target Stores building into the cat's `Inventory`.
/// Paired with a subsequent `Cook` step that flips the cooked
/// flag on the retrieved item.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Stores)`
/// with a follow-on `SetCarrying(Carrying::RawFood)` effect in
/// `src/ai/planner/actions.rs::cooking_actions`. `ZoneIs` alone
/// does not guarantee raw food is actually present — the planner's
/// `Carrying` state is a coarse projection of the multi-slot
/// `Inventory` (computed by `Carrying::from_inventory`).
///
/// **Runtime preconditions** — waits `ticks >= 5`. Requires
/// `target_entity` to resolve to a `StoredItems`, and for at least
/// one stored item to satisfy `kind.is_food() && !modifiers.cooked`.
/// Inventory must have a free slot — ticket 175 routes the
/// transfer through `components::item_transfer::transfer_item_stores_to_inventory`,
/// which encodes "items are real" by checking `Inventory` capacity
/// before calling `stored.remove` / `commands.entity(_).despawn()`.
/// On capacity miss the step returns
/// `unwitnessed(Fail("inventory full"))` so the cat re-plans
/// rather than silently destroying a real item entity. (Mirrors
/// `resolve_gather_herb`'s capacity-fail pattern at
/// `src/steps/magic/gather_herb.rs:54`.)
/// On no-target / Stores-not-found / no-matching-item: returns
/// `unwitnessed(Advance)` — the chain moves on (the substrate said
/// food was available but the cat arrived after another cat
/// claimed it).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff an item was
/// actually transferred from Stores to inventory this call.
///
/// **Feature emission** — caller passes `Feature::ItemRetrieved`
/// (Positive) to `record_if_witnessed`. Before §Phase 5a the
/// caller at `goap.rs:2657` discarded the retrieval bool
/// (`let (result, _retrieved) = …`) — Cook plans that began with a
/// missing-Stores retrieval fired no Feature and left the
/// retrieval pipeline invisible to the Activation canary.
pub fn resolve_retrieve_raw_food_from_stores(
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
        items_query
            .get(e)
            .is_ok_and(|item| item.kind.is_food() && !item.modifiers.cooked)
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
        // 175: pre-fix this path silently destroyed the item
        // (stored.remove + despawn ran regardless of the
        // inventory.add return). The contract now keeps the
        // item in Stores; the cat re-plans (likely electing
        // Eating or Hunting once their inventory clears).
        Err(TransferError::DestinationFull) => {
            StepOutcome::unwitnessed(StepResult::Fail("inventory full".into()))
        }
    }
}
