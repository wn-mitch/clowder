use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::item_transfer::{transfer_item_stores_to_inventory, TransferError};
use crate::components::items::Item;
use crate::components::magic::Inventory;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `RetrieveSmokeable`
///
/// **Real-world effect** — transfers up to two items from the target
/// Stores building into the cat's `Inventory`: one `is_raw_meat()` item
/// (RawMouse / RawRat / RawRabbit / RawBird) and one `is_fuel()` item
/// (Wood). Skips any ingredient the cat already carries. If both are
/// already in inventory, returns `unwitnessed(Advance)` so the chain
/// moves on to `SmokeMeat`. Paired with the subsequent `SmokeMeat` step
/// that consumes both items onto a Smoking Rack.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Stores)` with
/// a `SetCarrying(Carrying::RawFood)` effect in
/// `src/ai/planner/actions.rs::smoking_meat_actions`. The `RawFood`
/// search-state causally connects this step to `SmokeMeat`'s
/// `CarryingIs(RawFood)` precondition, ensuring A* always includes
/// the retrieve step.
///
/// **Runtime preconditions** — waits `ticks >= 5`. Requires
/// `target_entity` to resolve to a `StoredItems`. Inventory must have
/// a free slot for each ingredient not yet carried — the typed transfer
/// primitive checks capacity before removing from Stores so a full
/// inventory never silently destroys a real item. On no-target /
/// no-matching-item / Stores-not-found: returns `unwitnessed(Advance)`
/// so the chain moves on (another cat may have claimed the items).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff at least one item was
/// actually transferred from Stores to inventory this call.
///
/// **Feature emission** — caller passes `Feature::ItemRetrieved`
/// (Positive) to `record_if_witnessed`. The downstream `SmokeMeat`
/// step emits `Feature::MeatLoadedOnSmokingRack` when the load fires.
pub fn resolve_retrieve_smokeable_from_stores(
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

    let has_meat = inventory.has_raw_meat();
    let has_fuel = inventory.has_fuel();

    // Both already in inventory — advance without transferring.
    if has_meat && has_fuel {
        return StepOutcome::unwitnessed(StepResult::Advance);
    }

    let mut transferred = false;

    // Retrieve missing meat.
    if !has_meat {
        let meat_entity = stored.items.iter().copied().find(|&e| {
            items_query
                .get(e)
                .is_ok_and(|item| !item.modifiers.cooked && item.kind.is_raw_meat())
        });
        if let Some(meat_e) = meat_entity {
            if let Ok(item) = items_query.get(meat_e) {
                match transfer_item_stores_to_inventory(
                    &mut stored,
                    meat_e,
                    item.kind,
                    item.modifiers,
                    inventory,
                    commands,
                ) {
                    Ok(()) => {
                        transferred = true;
                    }
                    Err(TransferError::DestinationFull) => {
                        return StepOutcome::unwitnessed(StepResult::Fail(
                            "inventory full for smokeable meat".into(),
                        ));
                    }
                }
            }
        }
    }

    // Retrieve missing fuel.
    if !has_fuel {
        let fuel_entity = stored
            .items
            .iter()
            .copied()
            .find(|&e| items_query.get(e).is_ok_and(|item| item.kind.is_fuel()));
        if let Some(fuel_e) = fuel_entity {
            if let Ok(item) = items_query.get(fuel_e) {
                match transfer_item_stores_to_inventory(
                    &mut stored,
                    fuel_e,
                    item.kind,
                    item.modifiers,
                    inventory,
                    commands,
                ) {
                    Ok(()) => {
                        transferred = true;
                    }
                    Err(TransferError::DestinationFull) => {
                        return StepOutcome::unwitnessed(StepResult::Fail(
                            "inventory full for fuel".into(),
                        ));
                    }
                }
            }
        }
    }

    if transferred {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        // Stores didn't have the needed items; another cat claimed them.
        StepOutcome::unwitnessed(StepResult::Advance)
    }
}
