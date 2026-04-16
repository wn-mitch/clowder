use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::building::{ConstructionSite, CropState, Structure};
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::map::TileMap;
use crate::steps::StepResult;

#[allow(clippy::type_complexity)]
pub fn resolve_repair(
    target_entity: Option<Entity>,
    pos: &mut Position,
    cached_path: &mut Option<Vec<Position>>,
    skills: &mut Skills,
    workshop_bonus: f32,
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
    map: &TileMap,
) -> StepResult {
    let Some(target) = target_entity else {
        return StepResult::Fail("no target for Repair".into());
    };

    let Ok((_, mut structure, _, _, building_pos)) = buildings.get_mut(target) else {
        return StepResult::Fail("building not found".into());
    };

    if pos.manhattan_distance(building_pos) > 1 {
        if cached_path.is_none() {
            *cached_path = find_path(*pos, *building_pos, map);
        }
        if let Some(ref mut path) = cached_path {
            if !path.is_empty() {
                *pos = path.remove(0);
            }
        }
        return StepResult::Continue;
    }

    structure.condition = (structure.condition + skills.building * 0.01).min(1.0);
    skills.building += skills.growth_rate() * 0.01 * workshop_bonus;

    if structure.condition >= 1.0 {
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
