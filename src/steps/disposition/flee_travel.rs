use crate::ai::route_cost::CatPathPlan;
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::StepResult;

/// # GOAP step resolver: `Flee` umbrella travel (ticket 230)
///
/// **Real-world effect** — moves the cat one step toward the picked
/// flee target via the shared `CatPathPlan` machinery (gradient walk
/// over the per-replan `RouteCostField` when fresh; A\* fallback on
/// staleness, with `Feature::RouteCostFieldFallback` emission). On
/// reaching the target tile the resolver advances; otherwise it
/// continues. Mirrors the per-tick step shape of
/// `goap.rs::resolve_travel_to`'s gradient-walk arm but without the
/// zone-resolution / cached-path bookkeeping — `PickFleeTarget` has
/// already written `target_position` for this step, and the cat's
/// surroundings (fox-scent, corruption) shift fast enough that
/// per-tick gradient lookups are cheap and accurate.
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::FleeTargetPicked(true)` by
/// `src/ai/planner/actions.rs::fleeing_actions`.
///
/// **Runtime preconditions** — `target` is the position written by
/// `PickFleeTarget`. If the cat is already adjacent to the target,
/// the resolver snaps to it and returns `Advance`. Otherwise the
/// cat takes one step via `path_plan.next_step`. No internal hold,
/// no Fail path — if the path machinery refuses to move, the cat
/// stays put and the resolver returns `Continue`; the post-loop
/// stuck-detection in `resolve_goap_plans` catches chronic non-
/// progress and surfaces it as a plan failure.
///
/// **Witness** — `StepOutcome<()>`. The travel step has no
/// witness-bearing payload — it's a pure unconditional-effect
/// resolver in the same family as `resolve_move_to`. Feature
/// emission for the Fleeing chain happens at `PickFleeTarget`
/// (`FleeTargetPicked`) and `HoldUntilSafe` (`FleeRecovered`),
/// not here.
///
/// **Feature emission** — none directly. The umbrella's contribution
/// to the Fleeing chain's activation surface is mediated by the two
/// witness-bearing siblings.
pub fn resolve_flee_travel(
    pos: &mut Position,
    target: Position,
    path_plan: &CatPathPlan<'_>,
    map: &TileMap,
) -> StepResult {
    if *pos == target {
        return StepResult::Advance;
    }
    if pos.manhattan_distance(&target) <= 1 {
        *pos = target;
        return StepResult::Advance;
    }
    if let Some(next) = path_plan.next_step(*pos, target, map) {
        *pos = next;
    }
    if *pos == target || pos.manhattan_distance(&target) <= 1 {
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
