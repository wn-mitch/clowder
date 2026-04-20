use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::items::Item;
use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

pub fn resolve_eat_at_stores(
    ticks: u64,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<&Item>,
    commands: &mut Commands,
    d: &DispositionConstants,
) -> StepResult {
    if ticks >= d.eat_at_stores_duration {
        if let Some(store_entity) = target_entity {
            if let Ok(mut stored) = stores_query.get_mut(store_entity) {
                let food_item = stored.items.iter().copied().find(|&item_e| {
                    items_query
                        .get(item_e)
                        .is_ok_and(|item| item.kind.is_food())
                });
                if let Some(item_entity) = food_item {
                    if let Ok(item) = items_query.get(item_entity) {
                        let freshness = 1.0 - item.modifiers.corruption * d.corruption_food_penalty;
                        let cooked_mult = if item.modifiers.cooked {
                            d.cooked_food_multiplier
                        } else {
                            1.0
                        };
                        needs.hunger = (needs.hunger
                            + item.kind.food_value() * freshness * cooked_mult)
                            .min(1.0);
                    }
                    stored.remove(item_entity);
                    commands.entity(item_entity).despawn();
                }
            }
        }
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
