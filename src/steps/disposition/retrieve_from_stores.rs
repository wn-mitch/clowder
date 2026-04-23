use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
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
/// `target_entity` to resolve to a `StoredItems`, and for a
/// matching `ItemKind` to be present. Any miss returns
/// `unwitnessed(Advance)`: the chain moves on rather than
/// stalling on a now-empty store.
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
    items_query: &Query<&Item>,
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
    if let Some(item_entity) = target_item {
        let modifiers = items_query
            .get(item_entity)
            .map(|item| item.modifiers)
            .unwrap_or_default();
        stored.remove(item_entity);
        inventory.add_item_with_modifiers(kind, modifiers);
        commands.entity(item_entity).despawn();
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Advance)
    }
}
