use crate::ai::route_cost::CatPathPlan;
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::outcome::StepOutcome;
use crate::steps::StepResult;
use bevy_ecs::entity::Entity;

/// Witness payload for the Flee umbrella travel resolver — recorded
/// only when the step advances (i.e. the cat reached the picked flee
/// target this tick). Carries the threat the cat is fleeing from so
/// the caller can emit `WitnessableEvent::FleeFrom` without re-deriving
/// it from scratch.
///
/// 295: introduced when `resolve_flee_travel` was promoted from
/// `StepResult` to `StepOutcome<FleeWitness>` to give the belief
/// substrate (258) a real emit point. Before 295 the Fleeing chain's
/// activation surface was carried entirely by `PickFleeTarget` and
/// `HoldUntilSafe`; this struct is read-only data, the resolver does
/// not use `threat` for any movement logic.
#[derive(Debug, Clone, Copy)]
pub struct FleeWitness {
    pub threat: Entity,
}

/// # GOAP step resolver: `Flee` umbrella travel (ticket 230, witness-bearing per 295)
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
/// **Witness** — `StepOutcome<Option<FleeWitness>>`. `Some` when the
/// cat reached the flee target this tick AND the caller passed a
/// `Some(threat)` to attribute the flee to. `None` while still walking
/// OR if no threat entity was available at the caller (e.g. wildlife
/// transiently empty mid-flee — the cat still completes the step, but
/// there is no observably-correct threat to record for the belief
/// substrate, so the emit is skipped). The witness carries the
/// `threat` Entity passed by the caller — `belief_integrator` reads
/// this to update the witness's `MentalModel<Cat>.predictability`
/// (on the fleer) and `MentalModel<Predator>.perceived_violence_capability`
/// (on the threat).
///
/// **Feature emission** — none directly. The umbrella's contribution
/// to the Fleeing chain's activation surface is still mediated by the
/// two sibling steps (`FleeTargetPicked` at `PickFleeTarget`,
/// `FleeRecovered` at `HoldUntilSafe`). 295 adds a parallel
/// `WitnessableEvent::FleeFrom` emit at the caller, gated on this
/// resolver's witness.
pub fn resolve_flee_travel(
    pos: &mut Position,
    target: Position,
    threat: Option<Entity>,
    path_plan: &CatPathPlan<'_>,
    map: &TileMap,
    // 140 step 6 — flee expresses desire; the Chain-4 integrator does
    // the moving (momentum makes real flee arcs). The pre-140
    // snap-to-target teleport (`*pos = target` inside 1 tile) is
    // RETIRED — arrival is observed, not forced.
    desired: &mut crate::components::physical::DesiredVelocity,
    movement: &crate::resources::sim_constants::MovementConstants,
) -> StepOutcome<Option<FleeWitness>> {
    let reached = if *pos == target || pos.distance_to(&target) <= 1.0 {
        true
    } else {
        // Smoothed corridor per tick (flee targets re-pick frequently
        // and paths are short; recompute throttling is plan step 13).
        // Seek the first string-pulled waypoint at full speed; sprint
        // gait lands in step 12.
        let aim = path_plan
            .find_smoothed_path(*pos, target, map)
            .and_then(|path| path.first().copied())
            .unwrap_or(target);
        desired.0 = Some(crate::ai::steering::seek(
            pos.0,
            aim.0,
            movement.cat_max_speed,
        ));
        false
    };
    if !reached {
        return StepOutcome {
            result: StepResult::Continue,
            witness: None,
        };
    }
    // Reached this tick. Carry a witness only if the caller named a
    // threat — without one there is no FleeFrom event to emit, and the
    // belief integrator has nothing to attribute the predictability /
    // violence_capability updates to.
    match threat {
        Some(t) => StepOutcome::witnessed_with(StepResult::Advance, FleeWitness { threat: t }),
        None => StepOutcome {
            result: StepResult::Advance,
            witness: None,
        },
    }
}
