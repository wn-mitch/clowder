//! Fox step resolvers — execute individual planned fox actions.
//!
//! Each resolver takes the minimal state it needs, advances one tick of work,
//! and returns a [`StepResult`] signaling whether to continue, advance to the
//! next step, or fail (triggering replanning).

use crate::ai::pathfinding::find_path;
use crate::components::fox_goap_plan::FoxStepState;
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::StepResult;

// ---------------------------------------------------------------------------
// Movement helper
// ---------------------------------------------------------------------------

/// Step one tile along a cached A* path toward `target`. Returns true once
/// within `arrival_dist` of the target. Rebuilds path if cache empty.
pub fn step_toward(
    pos: &mut Position,
    target: Position,
    cached_path: &mut Option<Vec<Position>>,
    map: &TileMap,
    arrival_dist: i32,
) -> bool {
    if pos.manhattan_distance(&target) <= arrival_dist {
        return true;
    }
    if cached_path.is_none() {
        *cached_path = find_path(*pos, target, map);
    }
    if let Some(path) = cached_path {
        if !path.is_empty() {
            *pos = path.remove(0);
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Resolver: TravelTo
// ---------------------------------------------------------------------------

/// Walk one step toward an abstract zone's concrete position.
/// The target position is pre-resolved and stored in `step_state.target_position`.
pub fn resolve_travel_to(
    pos: &mut Position,
    step_state: &mut FoxStepState,
    map: &TileMap,
) -> StepResult {
    let Some(target) = step_state.target_position else {
        return StepResult::Fail("no target position for TravelTo".into());
    };
    if step_toward(pos, target, &mut step_state.cached_path, map, 1) {
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

/// Rest in place for a fixed number of ticks, restoring hunger satiation.
/// The caller is responsible for applying the hunger/satiation reset via
/// FoxState mutations since this is a pure-duration step.
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

/// Self-grooming — pure duration step.
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

/// Mark territory: deposits scent at current position (via FoxScentMap resource
/// update, handled by caller) and advances immediately.
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
        let mut pos = Position::new(5, 5);
        let mut state = FoxStepState {
            target_position: Some(Position::new(5, 5)),
            ..FoxStepState::default()
        };
        assert!(matches!(
            resolve_travel_to(&mut pos, &mut state, &map),
            StepResult::Advance
        ));
    }

    #[test]
    fn travel_fails_without_target() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(5, 5);
        let mut state = FoxStepState::default();
        assert!(matches!(
            resolve_travel_to(&mut pos, &mut state, &map),
            StepResult::Fail(_)
        ));
    }
}
