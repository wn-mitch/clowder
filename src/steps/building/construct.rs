use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::building::{ConstructionSite, CropState, Structure};
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::colony_score::ColonyScore;
use crate::resources::map::TileMap;
use crate::steps::StepResult;

/// Returns `(StepResult, should_continue)`. The `should_continue` flag indicates
/// the dispatcher should `continue` to the next entity (used when walking to site).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn resolve_construct(
    target_entity: Option<Entity>,
    pos: &mut Position,
    cached_path: &mut Option<Vec<Position>>,
    skills: &mut Skills,
    workshop_bonus: f32,
    builders_per_site: &HashMap<Entity, usize>,
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
    commands: &mut Commands,
    colony_score: &mut Option<ResMut<ColonyScore>>,
) -> StepResult {
    let Some(target) = target_entity else {
        return StepResult::Fail("no target for Construct".into());
    };

    let Ok((_, _, maybe_site, _, building_pos)) = buildings.get_mut(target) else {
        return StepResult::Fail("construction site not found".into());
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

    if let Some(mut site) = maybe_site {
        if !site.materials_complete() {
            return StepResult::Fail("materials not delivered".into());
        }

        let other_builders = builders_per_site.get(&target).copied().unwrap_or(1).max(1);
        let rate = skills.building * 0.02 * (1.0 + 0.3 * (other_builders as f32 - 1.0));
        site.progress = (site.progress + rate).min(1.0);
        skills.building += skills.growth_rate() * 0.01 * workshop_bonus;

        if site.progress >= 1.0 {
            let blueprint = site.blueprint;
            commands.entity(target).remove::<ConstructionSite>();
            commands.entity(target).insert(Structure::new(blueprint));
            if let Some(ref mut score) = colony_score {
                score.structures_built += 1;
            }
            StepResult::Advance
        } else {
            StepResult::Continue
        }
    } else {
        // ConstructionSite already removed — building is done.
        StepResult::Advance
    }
}
