use bevy_ecs::prelude::*;

use crate::components::building::{StoredItems, StructureType};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::{Inventory, ItemSlot};
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
    /// No Stores building exists; food was dropped on the ground.
    pub no_store: bool,
}

/// # GOAP step resolver: `DepositAtStores`
///
/// **Real-world effect** — transfers food items from the actor's
/// `Inventory` into the target `StoredItems`. When no Stores
/// exists, drops food on the ground at the actor's position (a
/// fallback so cats aren't forced to carry indefinitely). Tracks
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
    cat_pos: &Position,
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

    // No store exists — drop food on the ground at the cat's position.
    if target_entity.is_none() {
        let food_items: Vec<(ItemKind, crate::components::items::ItemModifiers)> = inventory
            .slots
            .iter()
            .filter_map(|slot| match slot {
                ItemSlot::Item(kind, mods) if kind.is_food() => Some((*kind, *mods)),
                _ => None,
            })
            .collect();

        if food_items.is_empty() {
            return DepositResult {
                step: StepResult::Advance,
                storage_upgraded: false,
                rejected: false,
                no_store: false,
            };
        }

        inventory
            .slots
            .retain(|slot| !matches!(slot, ItemSlot::Item(k, _) if k.is_food()));

        let quality = (d.deposit_quality_base + skills.hunting * d.deposit_quality_skill_scale)
            .clamp(0.0, 1.0);
        for (kind, mods) in food_items {
            commands.spawn((
                Item::with_modifiers(kind, quality, ItemLocation::OnGround, mods),
                *cat_pos,
            ));
        }
        return DepositResult {
            step: StepResult::Advance,
            storage_upgraded: false,
            rejected: false,
            no_store: true,
        };
    }

    let store_entity = target_entity.unwrap();
    let food_items: Vec<(ItemKind, crate::components::items::ItemModifiers)> = inventory
        .slots
        .iter()
        .filter_map(|slot| match slot {
            ItemSlot::Item(kind, mods) if kind.is_food() => Some((*kind, *mods)),
            _ => None,
        })
        .collect();
    // Remove deposited items from inventory up front.
    inventory
        .slots
        .retain(|slot| !matches!(slot, ItemSlot::Item(k, _) if k.is_food()));
    // Spawn real item entities in the store.
    if let Ok(mut stored) = stores_query.get_mut(store_entity) {
        let quality = (d.deposit_quality_base + skills.hunting * d.deposit_quality_skill_scale)
            .clamp(0.0, 1.0);
        for (kind, mods) in food_items {
            let item_entity = commands
                .spawn(Item::with_modifiers(
                    kind,
                    quality,
                    ItemLocation::StoredIn(store_entity),
                    mods,
                ))
                .id();
            if !stored.add_effective(item_entity, StructureType::Stores, items_query) {
                // Store is full — despawn the entity we just spawned.
                commands.entity(item_entity).despawn();
                rejected = true;
                break;
            }
            if kind.capacity_bonus() > 0 {
                storage_upgraded = true;
            }
        }
    }
    DepositResult {
        step: StepResult::Advance,
        storage_upgraded,
        rejected,
        no_store: false,
    }
}
