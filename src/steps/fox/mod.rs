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
