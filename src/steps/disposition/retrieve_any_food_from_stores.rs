use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::items::Item;
use crate::components::magic::Inventory;
use crate::steps::StepResult;

/// Retrieve any food item (raw or cooked) from the target Stores building
/// into the cat's inventory. Sibling to `resolve_retrieve_raw_food_from_stores`
/// but without the `!cooked` filter — Caretake / FeedKitten accepts either
/// form. Phase 4c.4 (wired after discovering Phase 4c.3 only fixed the
/// disposition-chain path; the scheduled GOAP Caretake plan was still two
/// steps `[TravelTo(Stores), FeedKitten]` with no retrieval step, causing
/// `take_food()` in `resolve_feed_kitten` to return `None` every time and
/// the kitten-feed to silently no-op — hence `KittenFed = 0` across both
/// seed-42 soaks even when bond-boost made adults pick Caretake).
///
/// Same ~5-tick budget as the raw-only sibling. Returns `(result, retrieved)`
/// where `retrieved == true` means an item transferred.
pub fn resolve_retrieve_any_food_from_stores(
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
    let target_item = stored
        .items
        .iter()
        .copied()
        .find(|&e| items_query.get(e).is_ok_and(|item| item.kind.is_food()));
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
