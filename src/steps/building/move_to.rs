use crate::ai::route_cost::CatPathPlan;
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `MoveTo`
///
/// **Real-world effect** — paths the actor toward `target_position`
/// one tile per tick via `CatPathPlan` (gradient-walk over the cat's
/// `RouteCostField` with A\* fallback when stale or absent); on
/// arrival (and when stacked), jitters to a free neighbor so cats
/// don't pile on destination tiles.
///
/// **Plan-level preconditions** — emitted by the builder /
/// task-chain planners as a pathfinding primitive; GOAP `TravelTo`
/// uses a different path.
///
/// **Runtime preconditions** — requires `target_position` to be
/// `Some` (Fail otherwise) and for pathfinding to succeed (Fail
/// otherwise). No silent-advance surface.
///
/// **Witness** — `StepOutcome<()>`. Movement is deterministic;
/// Advance means arrived, Continue means mid-path, Fail means
/// unreachable.
///
/// **Feature emission** — none. Movement is too ubiquitous to
/// track as a Positive Feature on its own.
pub fn resolve_move_to(
    pos: &mut Position,
    target_position: Option<Position>,
    cached_path: &mut Option<Vec<Position>>,
    map: &TileMap,
    path_plan: &CatPathPlan<'_>,
    desired: &mut crate::components::physical::DesiredVelocity,
    movement: &crate::resources::sim_constants::MovementConstants,
) -> StepOutcome<()> {
    let Some(target) = target_position else {
        return StepOutcome::bare(StepResult::Fail("no target position for MoveTo".into()));
    };
    if *pos == target {
        // 140 step 7 — the arrival anti-stack jitter-teleport is
        // RETIRED; co-located cats drift apart via the separation
        // desire pass (`movement::apply_separation`) instead.
        return StepOutcome::bare(StepResult::Advance);
    }
    if cached_path.is_none() {
        match path_plan.find_smoothed_path(*pos, target, map) {
            Some(path) => *cached_path = Some(path),
            None => return StepOutcome::bare(StepResult::Fail("no path to target".into())),
        }
    }
    if let Some(ref mut path) = cached_path {
        while let Some(wp) = path.first().copied() {
            if pos.0.distance(wp.0) <= movement.waypoint_arrival_radius {
                path.remove(0);
            } else {
                break;
            }
        }
        if path.is_empty() {
            StepOutcome::bare(StepResult::Advance)
        } else {
            desired.0 = Some(crate::ai::steering::seek(
                pos.0,
                path[0].0,
                movement.cat_max_speed,
            ));
            StepOutcome::bare(StepResult::Continue)
        }
    } else {
        StepOutcome::bare(StepResult::Continue)
    }
}
