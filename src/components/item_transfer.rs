//! # Items-are-real coding contract (Ticket 175)
//!
//! Real items in Clowder are real `Entity`s with an `Item` component
//! and an `ItemKind` + `ItemModifiers`. They live in one of three
//! places: a building's `StoredItems::items` Vec, a cat's
//! `Inventory::slots` Vec (as a value-typed `(kind, modifiers)`
//! `ItemSlot::Item(...)`), or on the ground. Moving an item between
//! those locations is a *transfer*; the cardinal rule is that **no
//! transfer may silently destroy the item**.
//!
//! Pre-175, `resolve_retrieve_raw_food_from_stores` ran the sequence
//! `stored.remove(...) → inventory.add_*(...)  → commands.entity(_).despawn()`
//! and **discarded the `add_*` return value**. When the cat's
//! inventory was full, the item was removed from Stores and despawned
//! but never added to inventory — the item entity was silently
//! destroyed. Pre-175 the planner-side `CarryingIs(Carrying::Nothing)`
//! veto kept this path unreachable for any cat with a non-Nothing
//! carry projection; ticket 175 (drop-the-veto + L2 carry-affinity)
//! made the path live.
//!
//! ## The contract
//!
//! This module is the **only supported way** to move items between
//! Stores buildings and cat inventories. The function signatures
//! encode the ordering invariant — the destination `add` runs first;
//! if it returns `false` (capacity), the source is left untouched
//! and the caller receives `Err`. There is no path through these
//! functions that despawns an item without proving the destination
//! accepted it.
//!
//! Three layers of enforcement:
//!
//! 1. **Type-level (this module)** — the transfer primitive's body is
//!    correct by construction; callers cannot get to the despawn
//!    without successfully adding to the destination.
//! 2. **Visibility-restricted destructive ops** — `StoredItems::remove`
//!    is `pub` today; ticket 175 keeps it `pub` for backward compat
//!    but the `just check` lint at layer 3 flags any new use outside
//!    this module's allowlist.
//! 3. **`just check` lint** (`scripts/check_item_transfers.sh`) —
//!    flags `stored.remove(...)` co-located with
//!    `commands.entity(...).despawn()` in the same function body
//!    that doesn't go through `transfer_item_*`. Allowlisted
//!    exceptions live in `scripts/item_transfers.allowlist`.
//!
//! Mirrors the existing repo precedents:
//!
//! - `StepOutcome<W>` + `record_if_witnessed` (`src/steps/outcome.rs`)
//!   makes silent-advance-without-effect a type error.
//! - `scripts/check_substrate_stubs.sh` makes marker-without-reader-
//!   or-writer a `just check` failure.
//!
//! New entity-transfer surfaces (e.g., wagon-to-Stores during
//! founding, cat-to-cat handoff if it lands) MUST add a transfer
//! primitive here rather than open-coding the sequence.

use bevy::prelude::*;

use crate::components::building::StoredItems;
use crate::components::items::{ItemKind, ItemModifiers};
use crate::components::magic::Inventory;

/// What went wrong with a transfer attempt. The caller decides
/// whether to `Fail`, `Refuse`, or retry on a later tick.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TransferError {
    /// The destination container is at capacity. The source is
    /// left untouched.
    DestinationFull,
}

/// Move one item from a `Stores` building's `StoredItems` into a
/// cat's `Inventory`.
///
/// Ordering invariant (encoded by construction): the inventory
/// `add_item_with_modifiers` runs first. If it returns `false`
/// (capacity), `stored.remove` and `commands.entity(_).despawn()`
/// DO NOT run — the item entity stays a real `Item` in the
/// building's Vec. On success, the source remove + despawn runs
/// atomically from the resolver's perspective.
///
/// Returns `Ok(())` if the item is now in the cat's inventory and
/// the source entity is despawned. Returns `Err(DestinationFull)`
/// if the cat's inventory was at capacity; the caller should
/// surface this as a step `Fail`/`Refuse` so the cat re-plans.
pub fn transfer_item_stores_to_inventory(
    stored: &mut StoredItems,
    item_entity: Entity,
    kind: ItemKind,
    modifiers: ItemModifiers,
    inventory: &mut Inventory,
    commands: &mut Commands,
) -> Result<(), TransferError> {
    if !inventory.add_item_with_modifiers(kind, modifiers) {
        return Err(TransferError::DestinationFull);
    }
    stored.remove(item_entity);
    commands.entity(item_entity).despawn();
    Ok(())
}

// Inverse direction (cat Inventory → Stores) is intentionally
// not implemented in 175. The silent-loss site identified in 175
// is on the retrieve side; the deposit-side resolvers don't
// despawn an entity until after the inventory `take_*` succeeds
// (the existing pattern is "spawn entity from value-typed slot →
// add to Stores → if Stores full, the new entity is left
// OnGround"), which is a different kind of hazard. Migrating the
// deposit-side resolvers to a typed primitive lands as the
// `tickets/NNN-item-transfer-contract-migration.md` follow-on
// (per the 175 closeout commit).

// ---------------------------------------------------------------------------
// 176: Inverse direction (Inventory → StoredItems)
// ---------------------------------------------------------------------------

/// 176: move one item from a cat's `Inventory` slot at `slot_idx`
/// into a destination building's `StoredItems`. Used by the
/// Trash-at-midden resolver and (potentially) by future
/// deposit-side resolvers wishing the typed contract.
///
/// Ordering invariant: a fresh `Item` entity is spawned from the
/// value-typed inventory slot, then `StoredItems::add` is attempted.
/// On capacity-fail the spawned entity is despawned and the slot is
/// left untouched. On success the slot is removed and the entity
/// becomes the building's. The `kind` returned in `Err` lets the
/// caller log the failed transfer's kind without re-reading the slot.
///
/// `dest_kind` is the destination building's `StructureType` —
/// passed because `StoredItems::add` consults `Self::capacity(kind)`.
/// For `StructureType::Midden` capacity is `usize::MAX`, so the
/// `Err(DestinationFull)` branch is unreachable; for other building
/// kinds it represents real overflow.
///
/// Skips `ItemSlot::Herb(_)` and `ItemSlot::Remedy(_)` entries —
/// callers should resolve these via the existing herb/remedy
/// pathways. Returns `Err(DestinationFull)` for non-item slots so
/// the caller can pick a different slot.
pub fn transfer_item_inventory_to_stored(
    inventory: &mut Inventory,
    slot_idx: usize,
    dest_stored: &mut StoredItems,
    dest_kind: crate::components::building::StructureType,
    dest_position: crate::components::physical::Position,
    commands: &mut Commands,
) -> Result<Entity, TransferError> {
    use crate::components::items::{Item, ItemLocation};
    use crate::components::magic::ItemSlot;

    let (kind, modifiers) = match inventory.slots.get(slot_idx) {
        Some(ItemSlot::Item(k, m)) => (*k, *m),
        _ => return Err(TransferError::DestinationFull),
    };

    // Spawn the item entity at the destination's location with
    // `StoredIn(...)` location-tag — but we don't yet have the
    // building entity here; the caller passes `dest_position` so
    // the entity's `Position` component reads correctly for any
    // ground-location fallback. The location is updated to
    // `StoredIn(dest_entity)` by the caller after this returns,
    // since transferring the entity into a specific building
    // requires the building's `Entity`. (Stage-2 uses a thin
    // helper signature; future cleanup can fold the building
    // entity into this primitive.)
    let item_entity = commands
        .spawn((
            Item::with_modifiers(kind, 1.0, ItemLocation::OnGround, modifiers),
            dest_position,
        ))
        .id();

    if !dest_stored.add(item_entity, dest_kind) {
        commands.entity(item_entity).despawn();
        return Err(TransferError::DestinationFull);
    }

    inventory.slots.swap_remove(slot_idx);
    Ok(item_entity)
}

/// 176: drop one item from a cat's `Inventory` slot at `slot_idx`
/// onto the ground at `position`. Spawns a real `Item` entity with
/// `ItemLocation::OnGround`; the slot is removed only on successful
/// spawn. This primitive cannot fail on capacity (the ground has
/// none), but the signature returns `Result` for symmetry with
/// `transfer_item_inventory_to_stored` and to leave room for future
/// world-edge gates (e.g., out-of-map positions).
///
/// Used by the Drop resolver (`Action::Drop`) and as the fallback
/// for `engage_prey` / `forage_item` when the cat's inventory has
/// no room for a fresh catch (Stage 2 carcass-on-ground refactor).
pub fn transfer_item_inventory_to_ground(
    inventory: &mut Inventory,
    slot_idx: usize,
    position: crate::components::physical::Position,
    commands: &mut Commands,
) -> Result<Entity, TransferError> {
    use crate::components::items::{Item, ItemLocation};
    use crate::components::magic::ItemSlot;

    let (kind, modifiers) = match inventory.slots.get(slot_idx) {
        Some(ItemSlot::Item(k, m)) => (*k, *m),
        _ => return Err(TransferError::DestinationFull),
    };

    let item_entity = commands
        .spawn((
            Item::with_modifiers(kind, 1.0, ItemLocation::OnGround, modifiers),
            position,
        ))
        .id();

    inventory.slots.swap_remove(slot_idx);
    Ok(item_entity)
}

/// 176: hand one item from a source cat's `Inventory` slot to a
/// target cat's `Inventory`. Slot-to-slot (no entity spawn churn —
/// inventory slots are value-typed `(kind, modifiers)`).
///
/// Returns `Err(DestinationFull)` if the target inventory is at
/// capacity; the source slot stays put so the donor can re-plan.
pub fn transfer_item_inventory_to_inventory(
    source: &mut Inventory,
    slot_idx: usize,
    target: &mut Inventory,
) -> Result<(), TransferError> {
    use crate::components::magic::ItemSlot;

    if target.is_full() {
        return Err(TransferError::DestinationFull);
    }

    let added = match source.slots.get(slot_idx) {
        Some(ItemSlot::Item(kind, modifiers)) => {
            target.add_item_with_modifiers(*kind, *modifiers)
        }
        Some(ItemSlot::Herb(kind)) => target.add_herb(*kind),
        None => return Err(TransferError::DestinationFull),
    };
    if !added {
        return Err(TransferError::DestinationFull);
    }
    source.slots.swap_remove(slot_idx);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::Item;

    fn make_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app
    }

    /// Ticket 175: the contract's defining test. Pre-fix
    /// behavior silently destroyed the item; the contract
    /// must instead leave it in Stores when inventory is full.
    #[test]
    fn full_inventory_leaves_item_in_stores() {
        let mut app = make_app();
        // Spawn a real item entity in Stores.
        let item_entity = app
            .world_mut()
            .spawn(Item::with_modifiers(
                ItemKind::RawMouse,
                1.0,
                crate::components::items::ItemLocation::OnGround,
                ItemModifiers::default(),
            ))
            .id();
        let mut stored = StoredItems::default();
        stored.items.push(item_entity);

        // Fill cat's inventory to capacity.
        let mut inventory = Inventory::default();
        for _ in 0..Inventory::MAX_SLOTS {
            assert!(inventory.add_item(ItemKind::RawMouse));
        }
        assert!(inventory.is_full());

        // Attempt transfer.
        let mut commands_q = app.world_mut().commands();
        let result = transfer_item_stores_to_inventory(
            &mut stored,
            item_entity,
            ItemKind::RawMouse,
            ItemModifiers::default(),
            &mut inventory,
            &mut commands_q,
        );
        // Bevy `Commands` is a SystemParam that doesn't actually
        // own resources; the deferred queue is implicit. We
        // can't apply it here without a Schedule, but we don't
        // need to — the tests assert synchronous state on
        // `stored.items` and `inventory.slots`.

        // Contract assertion: the transfer refused, the item
        // entity is still in Stores, the inventory wasn't
        // overcommitted.
        assert_eq!(result, Err(TransferError::DestinationFull));
        assert_eq!(stored.items.len(), 1, "item must remain in Stores");
        assert_eq!(stored.items[0], item_entity, "same entity, not a clone");
        assert!(inventory.is_full(), "inventory unchanged");
    }

    /// Happy path: capacity available, transfer succeeds, source
    /// is consumed.
    #[test]
    fn transfer_succeeds_when_inventory_has_room() {
        let mut app = make_app();
        let item_entity = app
            .world_mut()
            .spawn(Item::with_modifiers(
                ItemKind::RawRat,
                1.0,
                crate::components::items::ItemLocation::OnGround,
                ItemModifiers::default(),
            ))
            .id();
        let mut stored = StoredItems::default();
        stored.items.push(item_entity);

        let mut inventory = Inventory::default();
        // 0/5 slots used.

        let mut commands_q = app.world_mut().commands();
        let result = transfer_item_stores_to_inventory(
            &mut stored,
            item_entity,
            ItemKind::RawRat,
            ItemModifiers::default(),
            &mut inventory,
            &mut commands_q,
        );

        assert_eq!(result, Ok(()));
        assert_eq!(stored.items.len(), 0, "item removed from Stores on success");
        assert!(!inventory.is_full());
        assert!(
            inventory.has_item(ItemKind::RawRat),
            "item now in cat inventory"
        );
        // The source entity is queued for despawn via Commands;
        // we don't apply the queue in this unit test (no
        // Schedule), so we can't assert the ECS state. The
        // assertions above prove the transfer's pre-despawn
        // contract.
    }
}
