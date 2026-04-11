use bevy_ecs::prelude::*;

use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::physical::Position;
use crate::steps::StepResult;

pub fn resolve_harvest(
    target_entity: Option<Entity>,
    pos: &Position,
    stores_list: &[(Entity, Position)],
    buildings: &mut Query<
        (
            Entity,
            &mut Structure,
            Option<&mut ConstructionSite>,
            Option<&mut CropState>,
            &Position,
        ),
        Without<crate::components::task_chain::TaskChain>,
    >,
    stored_items: &mut Query<&mut StoredItems>,
    commands: &mut Commands,
) -> StepResult {
    let Some(target) = target_entity else {
        return StepResult::Fail("no target for Harvest".into());
    };

    let Ok((_, _, _, maybe_crop, _)) = buildings.get_mut(target) else {
        return StepResult::Fail("garden not found".into());
    };

    if let Some(mut crop) = maybe_crop {
        let nearest_store = stores_list
            .iter()
            .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
            .map(|(e, _)| *e);
        if let Some(store_entity) = nearest_store {
            for kind in [ItemKind::Berries, ItemKind::Roots] {
                let item_entity = commands
                    .spawn(Item::new(kind, 0.9, ItemLocation::StoredIn(store_entity)))
                    .id();
                if let Ok(mut stored) = stored_items.get_mut(store_entity) {
                    stored.add(item_entity, StructureType::Stores);
                }
            }
        }
        crop.growth = 0.0;
        StepResult::Advance
    } else {
        StepResult::Fail("no CropState on garden".into())
    }
}
