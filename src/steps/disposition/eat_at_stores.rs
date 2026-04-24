use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::items::Item;
use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `EatAtStores`
///
/// **Real-world effect** — consumes one food item from the target
/// Stores building, restoring hunger on the cat by the item's
/// `food_value`, scaled by `(1 - corruption * corruption_food_penalty)`
/// and `cooked_food_multiplier` when the cooked flag is set.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Stores)` by
/// `src/ai/planner/actions.rs::eating_actions`. `ZoneIs` only checks
/// the coarse planner zone, not whether the Stores entity actually
/// has food — runtime must.
///
/// **Runtime preconditions** — waits `ticks >= eat_at_stores_duration`
/// (Continue until then). Requires `target_entity` to be `Some` and
/// resolve to a `StoredItems` component, and for at least one stored
/// item to return `kind.is_food()`. All three misses cause
/// `unwitnessed(Advance)`: the chain moves on so the cat can re-plan
/// next tick rather than starving in place.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff food was consumed
/// *and* hunger restoration was applied this call.
///
/// **Feature emission** — caller passes `Feature::FoodEaten`
/// (Positive) to `record_if_witnessed`. Before §Phase 5a there was
/// no Feature for eating — a blind spot the Starvation canary could
/// only see once the entire colony was starving.
pub fn resolve_eat_at_stores(
    ticks: u64,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<&Item>,
    commands: &mut Commands,
    d: &DispositionConstants,
) -> StepOutcome<bool> {
    if ticks < d.eat_at_stores_duration {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }

    let Some(store_entity) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let Ok(mut stored) = stores_query.get_mut(store_entity) else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };

    let Some(item_entity) = stored.items.iter().copied().find(|&item_e| {
        items_query
            .get(item_e)
            .is_ok_and(|item| item.kind.is_food())
    }) else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };

    if let Ok(item) = items_query.get(item_entity) {
        let freshness = 1.0 - item.modifiers.corruption * d.corruption_food_penalty;
        let cooked_mult = if item.modifiers.cooked {
            d.cooked_food_multiplier
        } else {
            1.0
        };
        needs.hunger = (needs.hunger + item.kind.food_value() * freshness * cooked_mult).min(1.0);
    }
    stored.remove(item_entity);
    commands.entity(item_entity).despawn();
    StepOutcome::witnessed(StepResult::Advance)
}
