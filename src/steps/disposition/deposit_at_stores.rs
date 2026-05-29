use bevy_ecs::prelude::*;

use crate::components::building::{StoredItems, StructureType};
use crate::components::items::{Item, ItemLocation};
use crate::components::magic::Inventory;
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

/// Deposit result flags for feature tracking in the caller.
pub struct DepositResult {
    pub step: StepResult,
    /// A storage-upgrade item was deposited (capacity_bonus > 0).
    pub storage_upgraded: bool,
    /// At least one item couldn't be deposited because the store was full.
    pub rejected: bool,
    /// No Stores building exists. Food is retained in inventory; this
    /// flag drives the `DepositFailedNoStore` feature for telemetry.
    /// Post-shuffle-fix this branch should be unreachable from the
    /// PickingUp DSE path (eligibility-gated on
    /// `HasFoodStorageAccessible`); a `debug_assert!` in the resolver
    /// surfaces any regression that re-introduces the call site.
    pub no_store: bool,
}

/// # GOAP step resolver: `DepositAtStores`
///
/// **Real-world effect** — transfers food items from the actor's
/// `Inventory` into the target `StoredItems`. When no Stores
/// exists, retains the food in inventory and sets the `no_store`
/// flag for telemetry (pre-shuffle-fix this branch dropped food
/// at the cat's tile, which re-latched `HasGroundCarcass` and
/// kicked off the early-game pickup-shuffle loop; the
/// `HasFoodStorageAccessible` eligibility gate on PickingUpDse
/// should keep us out of this branch in production). Tracks
/// three side-signals via `DepositResult`: a storage-upgrade item
/// landed, some items were rejected for capacity, or no-store
/// fallback fired.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(Stores)` by
/// `src/ai/planner/actions.rs::depositing_actions`.
///
/// **Runtime preconditions** — `target_entity` may be `None` (the
/// no-store path handles this explicitly). If the store exists
/// but has no capacity, items are rejected individually
/// (`rejected` flag set).
///
/// **Witness** — this resolver predates the `StepOutcome<W>`
/// convention; it returns a `DepositResult` struct with a
/// `StepResult` field plus three boolean side-signals the caller
/// routes to different Features (`StorageUpgraded`,
/// `DepositRejected`, `DepositFailedNoStore`). Unlike the single-
/// witness shape, deposit's three outcomes are simultaneous — a
/// single call can upgrade capacity, reject overflow, AND handle
/// no-store, so the design keeps the struct rather than
/// collapsing to a single witness.
///
/// **Feature emission** — caller at `src/systems/goap.rs::Deposit`
/// arm (and `src/systems/disposition.rs`) records
/// `Feature::StorageUpgraded` on `storage_upgraded`,
/// `Feature::DepositRejected` on `rejected`, and
/// `Feature::DepositFailedNoStore` on `no_store` — each gated on
/// the corresponding flag rather than on `StepResult::Advance`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_deposit_at_stores(
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    skills: &Skills,
    // Retained in the signature for symmetry with other deposit
    // resolvers and so a future "drop-at-specific-spot" no-store
    // strategy doesn't have to thread it back through the callers.
    // Unused since the shuffle fix removed the at-cat-tile drop.
    _cat_pos: &Position,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    commands: &mut Commands,
    d: &DispositionConstants,
) -> DepositResult {
    let mut storage_upgraded = false;
    let mut rejected = false;

    // No store exists — retain food in inventory and surface the
    // condition via `no_store` (drives `Feature::DepositFailedNoStore`
    // for telemetry; the coordinator's `pressure.no_store` is colony-
    // level and runs independently). Pre-shuffle-fix this branch
    // dropped food back at the cat's tile, which re-latched
    // `HasGroundCarcass`, re-eligibilized `PickingUpDse`, and produced
    // the early-game visual shuffle. With the `HasFoodStorageAccessible`
    // eligibility gate, the only way the planner reaches this branch
    // is a DSE that hauls food without that gate — none exist today.
    if target_entity.is_none() {
        let has_food = inventory.pouch.iter().any(|slot| slot.kind.is_food());
        debug_assert!(
            !has_food,
            "resolve_deposit_at_stores reached the no-store branch with food in inventory. \
             The HasFoodStorageAccessible gate on PickingUpDse should make this unreachable; \
             a regression or a new DSE has wired food into the deposit path without the gate."
        );
        return DepositResult {
            step: StepResult::Advance,
            storage_upgraded: false,
            rejected: false,
            no_store: has_food,
        };
    }

    let store_entity = target_entity.unwrap();
    // 175: defer the inventory removal until after Stores accepts
    // each item. Pre-175 the in-store path removed ALL food from
    // inventory up front, then bailed on the first capacity miss
    // (`break` at the `add_effective` failure) — every food item
    // past that point was silently destroyed. Items are real;
    // un-deposited items must remain in inventory so the cat
    // either deposits the rest later or finds another sink.
    let food_slot_indices: Vec<usize> = inventory
        .pouch
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| if slot.kind.is_food() { Some(i) } else { None })
        .collect();
    if let Ok(mut stored) = stores_query.get_mut(store_entity) {
        let quality = (d.deposit_quality_base + skills.hunting * d.deposit_quality_skill_scale)
            .clamp(0.0, 1.0);
        // Track which inventory indices were successfully
        // deposited so we can remove them after the batch (Vec
        // index stability requires we don't `swap_remove`
        // mid-iteration).
        let mut deposited: Vec<usize> = Vec::with_capacity(food_slot_indices.len());
        for slot_idx in food_slot_indices {
            // The pre-collection filter only matched food slots; if
            // concurrent mutation changed the kind out from under us,
            // skip silently.
            if !inventory.pouch[slot_idx].kind.is_food() {
                continue;
            }
            let slot = &inventory.pouch[slot_idx];
            let (kind, mods) = (slot.kind, slot.modifiers);
            let item_entity = commands
                .spawn(Item::with_modifiers(
                    kind,
                    quality,
                    ItemLocation::StoredIn(store_entity),
                    mods,
                ))
                .id();
            if !stored.add_effective(
                item_entity,
                kind.capacity_bonus(),
                StructureType::Stores,
                items_query,
            ) {
                // Stores at capacity — despawn the entity we
                // spawned, mark `rejected`, leave the inventory
                // slot intact, and stop trying. The caller can
                // re-plan; the food stays real in the cat's
                // inventory.
                commands.entity(item_entity).despawn();
                rejected = true;
                break;
            }
            if kind.capacity_bonus() > 0 {
                storage_upgraded = true;
            }
            deposited.push(slot_idx);
        }
        // Remove deposited slots in reverse-index order so each
        // `swap_remove` doesn't disturb earlier indices.
        deposited.sort_unstable_by(|a, b| b.cmp(a));
        for idx in deposited {
            inventory.pouch.swap_remove(idx);
        }
    }
    DepositResult {
        step: StepResult::Advance,
        storage_upgraded,
        rejected,
        no_store: false,
    }
}
