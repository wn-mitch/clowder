use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::items::Item;
use crate::components::physical::Needs;
use crate::steps::StepResult;

pub fn resolve_feed_kitten(
    ticks: u64,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<&Item>,
    commands: &mut Commands,
) -> StepResult {
    if ticks >= 10 {
        if let Some(store_entity) = target_entity {
            if let Ok(mut stored) = stores_query.get_mut(store_entity) {
                let food_item = stored.items.iter().copied().find(|&item_e| {
                    items_query
                        .get(item_e)
                        .is_ok_and(|item| item.kind.is_food())
                });
                if let Some(item_entity) = food_item {
                    stored.remove(item_entity);
                    commands.entity(item_entity).despawn();
                }
            }
        }
        needs.social = (needs.social + 0.05).min(1.0);
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
