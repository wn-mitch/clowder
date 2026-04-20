use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::items::Item;
use crate::components::magic::Inventory;
use crate::steps::StepResult;

/// Retrieve any raw (uncooked) food item from the target Stores building into
/// the cat's inventory. Returns `(result, retrieved)` where `retrieved` is
/// `true` if an item transferred. Runs on the same ~5-tick budget as
/// `resolve_retrieve_from_stores`.
pub fn resolve_retrieve_raw_food_from_stores(
    ticks: u64,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<&Item>,
    commands: &mut Commands,
) -> (StepResult, bool) {
    if ticks < 5 {
        return (StepResult::Continue, false);
    }
    let Some(store_entity) = target_entity else {
        return (StepResult::Advance, false);
    };
    let Ok(mut stored) = stores_query.get_mut(store_entity) else {
        return (StepResult::Advance, false);
    };
    let target_item = stored.items.iter().copied().find(|&e| {
        items_query
            .get(e)
            .is_ok_and(|item| item.kind.is_food() && !item.modifiers.cooked)
    });
    if let Some(item_entity) = target_item {
        if let Ok(item) = items_query.get(item_entity) {
            let kind = item.kind;
            let modifiers = item.modifiers;
            stored.remove(item_entity);
            inventory.add_item_with_modifiers(kind, modifiers);
            commands.entity(item_entity).despawn();
            return (StepResult::Advance, true);
        }
    }
    (StepResult::Advance, false)
}
