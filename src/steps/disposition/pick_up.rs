//! 176 PickUp resolver — `Action::PickUp` / `DispositionKind::PickingUp`.
//!
//! Inverse of Drop: a cat at a ground item's position takes the item
//! into their inventory. Load-bearing for the kill→carcass-on-ground→
//! pick-up flow that 176 introduces in `engage_prey`.

use bevy_ecs::prelude::*;

use crate::components::items::{BuildMaterialItem, Item};
use crate::components::magic::Inventory;
use crate::steps::{StepOutcome, StepResult};

/// Witness emitted on a successful pickup. Carries the item entity
/// that was added to inventory (and is queued for despawn — the
/// item is now value-typed in the inventory slot) so the caller
/// can record `Feature::ItemRetrieved` (or a future
/// `Feature::ItemPickedUp` if the surfaces split) and observe the
/// transfer in focal traces.
#[derive(Debug, Clone, Copy)]
pub struct PickUpOutcome {
    pub item_entity: Entity,
}

/// # GOAP step resolver: `PickUpItemFromGround`
///
/// **Real-world effect** — reads the target ground `Item` entity's
/// `kind` + modifiers, adds them to the cat's `Inventory` as a
/// new slot, then despawns the ground entity. The ordering here
/// mirrors `transfer_item_stores_to_inventory` (175): the
/// inventory `add` runs first; if it fails on capacity the source
/// entity is left untouched.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(MaterialPile)` by `picking_up_actions` as a stage-2
/// placeholder zone (a dedicated `PlannerZone::TargetGroundItem`
/// ships with stage 3). The L2 PickingUp DSE picks the target item
/// entity and threads it as `target_entity`.
///
/// **Runtime preconditions** — `target_entity` must resolve to an
/// `Item` entity (not despawned, not picked up by another cat). The
/// query filter excludes `BuildMaterialItem`s — those move through
/// the haul-to-construction-site pipeline, not the disposal chain;
/// a build-material target Fails the step. The cat must be at the
/// item's tile (the planner-side zone resolution provides this
/// approximately; the resolver tolerates adjacent positions). The
/// cat's inventory must have room.
///
/// **Witness** — `StepOutcome<Option<PickUpOutcome>>`. `Some(outcome)`
/// on `Advance` carries the picked-up item entity. `None` on `Fail`
/// (item gone, inventory full, target unset).
///
/// **Feature emission** — caller passes `Feature::ItemRetrieved`
/// (Positive) to `record_if_witnessed`. Reuses the existing
/// retrieved-from-stores Feature for now; a future surface split
/// can introduce a dedicated `ItemPickedUp` if focal-trace
/// distinctness becomes load-bearing.
pub fn resolve_pick_up_from_ground(
    inventory: &mut Inventory,
    target_entity: Option<Entity>,
    items: &Query<&Item, bevy_ecs::query::Without<BuildMaterialItem>>,
    commands: &mut Commands,
) -> StepOutcome<Option<PickUpOutcome>> {
    let Some(item_entity) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "pick_up: no target item on disposition".to_string(),
        ));
    };

    let Ok(item) = items.get(item_entity) else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "pick_up: target item entity not found (already picked up?)".to_string(),
        ));
    };

    if !inventory.add_item_with_modifiers(item.kind, item.modifiers) {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "pick_up: inventory full".to_string(),
        ));
    }

    commands.entity(item_entity).despawn();
    StepOutcome::witnessed_with(
        StepResult::Advance,
        PickUpOutcome { item_entity },
    )
}
