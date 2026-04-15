use bevy_ecs::prelude::*;

use crate::components::building::{StoredItems, StructureType};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::{Inventory, ItemSlot};
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
}

pub fn resolve_deposit_at_stores(
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    skills: &Skills,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<&Item>,
    commands: &mut Commands,
    d: &DispositionConstants,
) -> DepositResult {
    let mut storage_upgraded = false;
    let mut rejected = false;

    if let Some(store_entity) = target_entity {
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
    }
    DepositResult {
        step: StepResult::Advance,
        storage_upgraded,
        rejected,
    }
}
