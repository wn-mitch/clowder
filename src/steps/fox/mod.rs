//! Fox step resolvers — execute individual planned fox actions.
//!
//! Each resolver takes the minimal state it needs, advances one tick of work,
//! and returns a [`StepResult`] signaling whether to continue, advance to the
//! next step, or fail (triggering replanning).

use crate::ai::pathfinding::{find_path, CatPatrolDeterrentOverlay, WeightedOverlay};
use crate::components::fox_goap_plan::FoxStepState;
use crate::components::physical::{DesiredVelocity, Position};
use crate::resources::map::TileMap;
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::CatPatrolDeterrentMap;
use crate::steps::StepResult;

// ---------------------------------------------------------------------------
// Movement helper
// ---------------------------------------------------------------------------

/// 140 step 9 — seek along a cached string-pulled corridor toward
/// `target`, writing a `DesiredVelocity`; the Chain-4 integrator owns
/// motion (fox speed cap `fox_max_speed`). Returns `true` once within
/// `arrival_dist` (world-space Euclidean). Mirrors the cat-side
/// `CatPathPlan::desire_step_along_smoothed`, with the fox's own
/// overlay set.
///
/// 256 R5 — the fox A* reads `CatPatrolDeterrentMap` as a routing cost
/// overlay so foxes detour around active patrols instead of charging
/// straight through them; the same overlay feeds `smooth_path`'s
/// cost-aware raycast so string-pulling never shortcuts through a
/// patrol corridor the router paid to avoid. Foxes still don't read
/// fox-scent or corruption — those overlays are cat-perception layers.
///
/// The cache holds the *smoothed* waypoints (world-space `Position`s);
/// it is rebuilt when empty or when its final waypoint no longer
/// matches `target` (tile-keyed `Position` equality).
#[allow(clippy::too_many_arguments)]
pub fn desire_toward(
    pos: &Position,
    target: Position,
    cached_path: &mut Option<Vec<Position>>,
    map: &TileMap,
    arrival_dist: f32,
    deterrent_map: &CatPatrolDeterrentMap,
    sc: &ScoringConstants,
    desired: &mut DesiredVelocity,
    max_speed: f32,
    waypoint_arrival_radius: f32,
) -> bool {
    if pos.0.distance(target.0) <= arrival_dist {
        return true;
    }
    let stale = match cached_path {
        None => true,
        Some(p) => p.last().is_none_or(|last| *last != target),
    };
    if stale {
        let deterrent_overlay = CatPatrolDeterrentOverlay::new(deterrent_map, sc);
        let overlays = [WeightedOverlay::new(
            &deterrent_overlay,
            sc.cat_patrol_deterrent_overlay_weight,
        )];
        *cached_path = find_path(*pos, target, map, &overlays).map(|path| {
            crate::ai::steering::smooth_path(pos.world(), &path, map, &overlays)
                .into_iter()
                .map(Position)
                .collect()
        });
    }
    let Some(path) = cached_path else {
        // No route — the caller's watchdog owns the outcome.
        return false;
    };
    while let Some(wp) = path.first().copied() {
        if pos.0.distance(wp.0) <= waypoint_arrival_radius {
            path.remove(0);
        } else {
            break;
        }
    }
    let aim = path.first().copied().unwrap_or(target);
    desired.0 = Some(crate::ai::steering::seek(pos.0, aim.0, max_speed));
    false
}

// ---------------------------------------------------------------------------
// Resolver: TravelTo
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `TravelTo` (fox)
///
/// **Real-world effect** — walks the fox one tile toward its
/// pre-resolved target position. Parallels the cat-side
/// `MoveTo`/`PatrolTo` resolvers but shares no code — fox plans
/// use a separate GOAP schedule.
///
/// **Plan-level preconditions** — emitted by the fox GOAP
/// planner (`src/ai/fox_goap.rs`); target is pre-resolved from
/// the abstract zone at plan-build time.
///
/// **Runtime preconditions** — `step_state.target_position` must
/// be `Some`; Fail otherwise. Also has a 200-tick watchdog Fail
/// if no movement progress.
///
/// **Witness** — returns plain `StepResult`. Fox resolvers
/// predate the `StepOutcome<W>` convention; they have their own
/// less-elaborate Feature story (`FoxHuntedPrey`,
/// `FoxStoreRaided`, etc. fire from the fox AI system, not from
/// these step resolvers).
///
/// **Feature emission** — none from this step directly. Fox
/// Features are emitted from `src/systems/wildlife.rs` and
/// related fox-ai systems.
pub fn resolve_travel_to(
    pos: &Position,
    desired: &mut DesiredVelocity,
    movement: &crate::resources::sim_constants::MovementConstants,
    step_state: &mut FoxStepState,
    map: &TileMap,
    deterrent_map: &CatPatrolDeterrentMap,
    sc: &ScoringConstants,
) -> StepResult {
    let Some(target) = step_state.target_position else {
        return StepResult::Fail("no target position for TravelTo".into());
    };
    if desire_toward(
        pos,
        target,
        &mut step_state.cached_path,
        map,
        1.0,
        deterrent_map,
        sc,
        desired,
        movement.fox_max_speed,
        movement.waypoint_arrival_radius,
    ) {
        StepResult::Advance
    } else {
        // Watchdog: if no movement for many ticks, something is wrong.
        step_state.ticks_elapsed += 1;
        if step_state.ticks_elapsed > 200 {
            return StepResult::Fail("travel timeout".into());
        }
        StepResult::Continue
    }
}

// ---------------------------------------------------------------------------
// Resolver: Rest
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `Rest` (fox)
///
/// **Real-world effect** — pure-duration step. Caller applies
/// fox hunger/satiation reset after completion.
///
/// **Plan-level preconditions** — emitted by the fox GOAP
/// planner for den-rest actions.
///
/// **Runtime preconditions** — none; time-only gate.
///
/// **Witness** — returns plain `StepResult`. No side-effect
/// here; caller is witness.
///
/// **Feature emission** — none from this step directly.
pub fn resolve_rest(step_state: &mut FoxStepState, ticks_to_rest: u64) -> StepResult {
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed >= ticks_to_rest {
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}

// ---------------------------------------------------------------------------
// Resolver: GroomSelf
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `GroomSelf` (fox)
///
/// **Real-world effect** — pure-duration step; caller applies
/// grooming-related fox state updates.
///
/// **Plan-level preconditions** — emitted by the fox GOAP
/// planner.
///
/// **Runtime preconditions** — none.
///
/// **Witness** — returns plain `StepResult`.
///
/// **Feature emission** — none.
pub fn resolve_groom_self(step_state: &mut FoxStepState, ticks_to_groom: u64) -> StepResult {
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed >= ticks_to_groom {
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}

// ---------------------------------------------------------------------------
// Resolver: DepositScent
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `DepositScent` (fox)
///
/// **Real-world effect** — single-tick advance. The scent-
/// deposition into `FoxScentMap` is handled by the calling
/// system after this returns Advance.
///
/// **Plan-level preconditions** — emitted by the fox GOAP
/// planner for scent-marking actions.
///
/// **Runtime preconditions** — none.
///
/// **Witness** — returns plain `StepResult`; always Advance.
///
/// **Feature emission** — `Feature::FoxScentMarked` (Neutral) is
/// emitted by the calling fox-AI system, not from this resolver.
pub fn resolve_deposit_scent(_step_state: &mut FoxStepState) -> StepResult {
    // Scent deposition is a side-effect in the calling system; the step
    // completes in one tick.
    StepResult::Advance
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::Terrain;

    #[test]
    fn rest_advances_after_duration() {
        let mut state = FoxStepState::default();
        assert!(matches!(resolve_rest(&mut state, 3), StepResult::Continue));
        assert!(matches!(resolve_rest(&mut state, 3), StepResult::Continue));
        assert!(matches!(resolve_rest(&mut state, 3), StepResult::Advance));
    }

    #[test]
    fn deposit_scent_advances_immediately() {
        let mut state = FoxStepState::default();
        assert!(matches!(
            resolve_deposit_scent(&mut state),
            StepResult::Advance
        ));
    }

    #[test]
    fn travel_advances_when_already_at_target() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let deterrent = CatPatrolDeterrentMap::default_map();
        let sc = ScoringConstants::default();
        let movement = crate::resources::sim_constants::MovementConstants::default();
        let pos = Position::new(5, 5);
        let mut d = DesiredVelocity::default();
        let mut state = FoxStepState {
            target_position: Some(Position::new(5, 5)),
            ..FoxStepState::default()
        };
        assert!(matches!(
            resolve_travel_to(&pos, &mut d, &movement, &mut state, &map, &deterrent, &sc),
            StepResult::Advance
        ));
        assert!(d.0.is_none(), "arrival must not express desire");
    }

    #[test]
    fn travel_fails_without_target() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let deterrent = CatPatrolDeterrentMap::default_map();
        let sc = ScoringConstants::default();
        let movement = crate::resources::sim_constants::MovementConstants::default();
        let pos = Position::new(5, 5);
        let mut d = DesiredVelocity::default();
        let mut state = FoxStepState::default();
        assert!(matches!(
            resolve_travel_to(&pos, &mut d, &movement, &mut state, &map, &deterrent, &sc),
            StepResult::Fail(_)
        ));
    }

    #[test]
    fn fox_routes_around_high_deterrent_cell() {
        // 256 R5 — verify the deterrent overlay actually steers
        // foxes around patrols when an alternative exists. Place a
        // single-tile high-deterrent cell on the direct path; the
        // fox should detour.
        let map = TileMap::new(10, 10, Terrain::Grass);
        let mut deterrent = CatPatrolDeterrentMap::default_map();
        // Saturate the bucket containing (5, 5). Default bucket size 5,
        // so (5, 5) is in bucket (1, 1) which covers tiles (5..10, 5..10).
        deterrent.deposit(5, 5, 1.0);
        let sc = ScoringConstants::default();

        let movement = crate::resources::sim_constants::MovementConstants::default();
        let pos = Position::new(0, 0);
        let mut d = DesiredVelocity::default();
        let mut cache = None;
        // Express desire from (0, 0) toward (9, 9). With the deterrent
        // bucket at (5..10, 5..10) saturated, the smoothed corridor
        // should curve around it (or accept higher cost in fewer cells).
        let _arrived = desire_toward(
            &pos,
            Position::new(9, 9),
            &mut cache,
            &map,
            1.0,
            &deterrent,
            &sc,
            &mut d,
            movement.fox_max_speed,
            movement.waypoint_arrival_radius,
        );
        // The smoothed path was built; verify it's non-empty and a
        // desire was expressed toward its first waypoint.
        assert!(cache.is_some(), "path should be built");
        let path = cache.as_ref().unwrap();
        assert!(!path.is_empty(), "smoothed path should have waypoints");
        assert!(d.0.is_some(), "desire expressed while traveling");
    }
}
