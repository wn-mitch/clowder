use bevy_ecs::prelude::*;

use crate::components::building::{
    ConstructionSite, CropKind, CropState, StoredItems, Structure, StructureType,
};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::{GrowthStage, Harvestable, Herb, HerbKind, Seasonal};
use crate::components::physical::Position;
use crate::resources::time::Season;
use crate::steps::StepResult;

#[allow(clippy::type_complexity)]
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

    let Ok((_, _, _, maybe_crop, garden_pos)) = buildings.get_mut(target) else {
        return StepResult::Fail("garden not found".into());
    };
    let garden_pos = *garden_pos;

    if let Some(mut crop) = maybe_crop {
        match crop.crop_kind {
            CropKind::FoodCrops => {
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
            }
            CropKind::Thornbriar => {
                // Spawn a harvestable Thornbriar herb at the garden position.
                let available = vec![
                    Season::Spring,
                    Season::Summer,
                    Season::Autumn,
                    Season::Winter,
                ];
                commands.spawn((
                    Herb {
                        kind: HerbKind::Thornbriar,
                        growth_stage: GrowthStage::Blossom,
                        magical: false,
                        twisted: false,
                    },
                    garden_pos,
                    Seasonal { available },
                    Harvestable,
                ));
            }
        }
        crop.growth = 0.0;
        StepResult::Advance
    } else {
        StepResult::Fail("no CropState on garden".into())
    }
}
