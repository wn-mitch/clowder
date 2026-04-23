use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::building::{ConstructionSite, CropState, Structure};
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `TendCrops`
///
/// **Real-world effect** — increments a Garden's `CropState.growth`
/// by `skills.foraging * season_mod * 0.01 * crop_modifier`. When
/// growth reaches ≥ 1.0, the step advances (the following
/// `HarvestCrops` step converts the mature crop to food items).
/// Also grows the cat's `foraging` skill.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Farm)` by
/// `src/ai/planner/actions.rs::farming_actions`. The planner does
/// not check for an actual Garden with a `CropState` — the runtime
/// resolver must.
///
/// **Runtime preconditions** — requires `target_entity` to resolve
/// to a building with both `Position` and `CropState`. If the
/// target is missing (`None` or entity despawned), returns
/// `unwitnessed(Fail("…"))` — the chain drops so the planner can
/// re-resolve. If the cat is > 1 tile away, paths toward the
/// garden and returns `unwitnessed(Continue)`: walking is not
/// tending. Only when adjacent-and-crop-present does the witness
/// flip to `true`.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff `CropState.growth`
/// was actually incremented this call. The crop may still be mid-
/// growth (`Continue`) or newly-ripe (`Advance`) — both carry the
/// witness, since both represent real tending work.
///
/// **Feature emission** — caller passes `Feature::CropTended`
/// (Positive) to `record_if_witnessed`. Before Phase 4c.4 no
/// Feature fired at all; 450+ silent `PlanStepFailed "no target for
/// Tend"` per soak hid a dead farming pipeline for weeks.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
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
) -> StepOutcome<bool> {
    let Some(target) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Fail("no target for Tend".into()));
    };

    let Ok((_, _, _, maybe_crop, garden_pos)) = buildings.get_mut(target) else {
        return StepOutcome::unwitnessed(StepResult::Fail("garden not found".into()));
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
        return StepOutcome::unwitnessed(StepResult::Continue);
    }

    let Some(mut crop) = maybe_crop else {
        return StepOutcome::unwitnessed(StepResult::Fail("no CropState on garden".into()));
    };

    let crop_modifier = match crop.crop_kind {
        crate::components::building::CropKind::Thornbriar => 0.5,
        _ => 1.0,
    };
    crop.growth += skills.foraging * season_mod * 0.01 * crop_modifier;
    skills.foraging += skills.growth_rate() * 0.005 * workshop_bonus;

    let result = if crop.growth >= 1.0 {
        StepResult::Advance
    } else {
        StepResult::Continue
    };
    StepOutcome::witnessed(result)
}
