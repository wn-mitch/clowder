use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::items::{Item, ItemKind};
use crate::components::magic::Inventory;
use crate::steps::StepResult;

pub fn resolve_retrieve_from_stores(
    ticks: u64,
    kind: ItemKind,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<&Item>,
    commands: &mut Commands,
) -> (StepResult, bool) {
    if ticks >= 5 {
        let mut retrieved = false;
        if let Some(store_entity) = target_entity {
            if let Ok(mut stored) = stores_query.get_mut(store_entity) {
                let target_item = stored
                    .items
                    .iter()
                    .copied()
                    .find(|&e| items_query.get(e).is_ok_and(|item| item.kind == kind));
                if let Some(item_entity) = target_item {
                    let modifiers = items_query
                        .get(item_entity)
                        .map(|item| item.modifiers)
                        .unwrap_or_default();
                    stored.remove(item_entity);
                    inventory.add_item_with_modifiers(kind, modifiers);
                    commands.entity(item_entity).despawn();
                    retrieved = true;
                }
            }
        }
        (StepResult::Advance, retrieved)
    } else {
        (StepResult::Continue, false)
    }
}
