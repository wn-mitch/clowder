use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::building::{ConstructionSite, CropState, Structure};
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::map::TileMap;
use crate::steps::StepResult;

pub fn resolve_tend(
    target_entity: Option<Entity>,
    pos: &mut Position,
    cached_path: &mut Option<Vec<Position>>,
    skills: &mut Skills,
    season_mod: f32,
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
        return StepResult::Fail("no target for Tend".into());
    };

    let Ok((_, _, _, maybe_crop, garden_pos)) = buildings.get_mut(target) else {
        return StepResult::Fail("garden not found".into());
    };

    if pos.manhattan_distance(garden_pos) > 1 {
        if cached_path.is_none() {
            *cached_path = find_path(*pos, *garden_pos, map);
        }
        if let Some(ref mut path) = cached_path {
            if !path.is_empty() {
                *pos = path.remove(0);
            }
        }
        return StepResult::Continue;
    }

    if let Some(mut crop) = maybe_crop {
        crop.growth += skills.foraging * season_mod * 0.01;
        skills.foraging += skills.growth_rate() * 0.005 * workshop_bonus;

        if crop.growth >= 1.0 {
            StepResult::Advance
        } else {
            StepResult::Continue
        }
    } else {
        StepResult::Fail("no CropState on garden".into())
    }
}
