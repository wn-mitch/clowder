use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::building::{ConstructionSite, CropState, Structure};
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Repair`
///
/// **Real-world effect** — paths the actor to a damaged
/// `Structure`, then every tick increments
/// `structure.condition` by `skills.building * 0.01` until it
/// reaches 1.0. Also grows the actor's building skill.
///
/// **Plan-level preconditions** — emitted by the builder-chain
/// planner in `src/systems/coordination.rs` when damaged
/// structures exist.
///
/// **Runtime preconditions** — `target_entity` must resolve to a
/// building; misses return Fail. Walking path returns Continue
/// until adjacent. No silent-advance surface.
///
/// **Witness** — `StepOutcome<bool>`. `true` on the tick that
/// actually pushed condition to ≥ 1.0 (the completion event);
/// `false` while walking or mid-repair. This is distinct from
/// a simple `()` shape so `Feature::BuildingRepaired` can fire
/// exactly once per repair rather than on every per-tick
/// condition increment.
///
/// **Feature emission** — caller passes
/// `Feature::BuildingRepaired` (Positive) to
/// `record_if_witnessed`. Before §Phase 5a there was no Feature —
/// repair work was invisible to the Activation canary.
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
) -> StepOutcome<bool> {
    let Some(target) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Fail("no target for Repair".into()));
    };

    let Ok((_, mut structure, _, _, building_pos)) = buildings.get_mut(target) else {
        return StepOutcome::unwitnessed(StepResult::Fail("building not found".into()));
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
        return StepOutcome::unwitnessed(StepResult::Continue);
    }

    structure.condition = (structure.condition + skills.building * 0.01).min(1.0);
    skills.building += skills.growth_rate() * 0.01 * workshop_bonus;

    if structure.condition >= 1.0 {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}
