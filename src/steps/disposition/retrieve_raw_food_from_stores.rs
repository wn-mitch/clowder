use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
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
/// `Carrying` state is a coarse abstraction.
///
/// **Runtime preconditions** — waits `ticks >= 5`. Requires
/// `target_entity` to resolve to a `StoredItems`, and for at least
/// one stored item to satisfy `kind.is_food() && !modifiers.cooked`.
/// Any miss returns `unwitnessed(Advance)`: the chain moves on
/// rather than stalling.
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
    if let Some(item_entity) = target_item {
        if let Ok(item) = items_query.get(item_entity) {
            let kind = item.kind;
            let modifiers = item.modifiers;
            stored.remove(item_entity);
            inventory.add_item_with_modifiers(kind, modifiers);
            commands.entity(item_entity).despawn();
            return StepOutcome::witnessed(StepResult::Advance);
        }
    }
    StepOutcome::unwitnessed(StepResult::Advance)
}
