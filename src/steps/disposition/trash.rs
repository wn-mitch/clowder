//! 176 Trash resolver — `Action::Trash` / `DispositionKind::Trashing`.
//!
//! Carries one item from a cat's inventory to the nearest Midden
//! building and adds it to the Midden's `StoredItems`. Midden
//! capacity is `usize::MAX`, so the deposit cannot fail on capacity
//! grounds; the resolver only Fails when the cat has no item to
//! trash or no Midden exists in range.

use bevy_ecs::prelude::*;

use crate::components::building::{StoredItems, StructureType};
use crate::components::item_transfer::transfer_item_inventory_to_stored;
use crate::components::magic::{Inventory, ItemSlot};
use crate::components::physical::Position;
use crate::steps::{StepOutcome, StepResult};

/// Witness emitted on a successful trash. Carries the destination
/// midden entity and the spawned (and now stored) item entity so
/// callers can record `Feature::ItemTrashed` and thread the entities
/// into focal-trace observability.
#[derive(Debug, Clone, Copy)]
pub struct TrashOutcome {
    pub midden_entity: Entity,
    pub item_entity: Entity,
}

/// # GOAP step resolver: `TrashItemAtMidden`
///
/// **Real-world effect** — spawns one `Item` entity at the Midden
/// building's position, adds it to the Midden's `StoredItems`, and
/// removes the corresponding slot from the cat's `Inventory`. The
/// step is instant on entry once the cat has arrived; the caller is
/// expected to have routed a `MoveTo(midden)` step before this one.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Wilds)` (the
/// stage-2 placeholder zone for Midden-resident actions; a
/// `PlannerZone::Midden` ships with stage 3). The L2 disposal DSE
/// picks the target Midden entity and threads it as
/// `target_entity` on the cat's `CurrentAction` /
/// `Disposition::target_entity`.
///
/// **Runtime preconditions** — caller has already validated the
/// target as a `StructureType::Midden` (177 wires this via the
/// `snaps.midden_entities` snapshot) and threaded the resolved
/// `&mut StoredItems` and midden `Position`. The cat's inventory
/// must hold at least one `ItemSlot::Item(...)`; otherwise Fail.
///
/// **Witness** — `StepOutcome<Option<TrashOutcome>>`. `Some(outcome)`
/// on `Advance` carries midden + item entities. `None` on `Fail`.
///
/// **Feature emission** — caller passes `Feature::ItemTrashed`
/// (Neutral) to `record_if_witnessed`.
pub fn resolve_trash_at_midden(
    inventory: &mut Inventory,
    midden_entity: Entity,
    stored: &mut StoredItems,
    midden_pos: Position,
    commands: &mut Commands,
) -> StepOutcome<Option<TrashOutcome>> {
    let Some(slot_idx) = inventory
        .slots
        .iter()
        .position(|s| matches!(s, ItemSlot::Item(_, _)))
    else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "trash: no item-slot in inventory".to_string(),
        ));
    };

    match transfer_item_inventory_to_stored(
        inventory,
        slot_idx,
        stored,
        StructureType::Midden,
        midden_pos,
        commands,
    ) {
        Ok(item_entity) => StepOutcome::witnessed_with(
            StepResult::Advance,
            TrashOutcome {
                midden_entity,
                item_entity,
            },
        ),
        Err(_) => StepOutcome::unwitnessed(StepResult::Fail(
            "trash: midden refused (capacity?) — should be unreachable for unlimited Midden"
                .to_string(),
        )),
    }
}
