//! Hawk step resolvers — execute individual planned hawk actions.
//!
//! Each resolver follows the §"GOAP Step Resolver Contract" in CLAUDE.md
//! and returns a [`StepOutcome<W>`] (`src/steps/outcome.rs`). Positive
//! `Feature::*` emission MUST go through `record_if_witnessed` at the
//! call site — this is enforced at the type level for shapes whose
//! witness type implements [`crate::steps::outcome::Witnessed`].
//!
//! Hawks move through the air, so the movement helper here ignores
//! terrain and steps diagonally. The fox-side `step_toward` (which
//! reads the cat-patrol deterrent overlay) is land-only and not
//! reused.

use bevy_ecs::prelude::Entity;

use crate::components::hawk_goap_plan::HawkStepState;
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

// ---------------------------------------------------------------------------
// Flight movement helper
// ---------------------------------------------------------------------------

/// Diagonal step toward `target`. Returns `true` once the hawk is within
/// `arrival_dist` tiles. Ignores terrain — hawks fly. Caller is
/// responsible for refreshing `target` when zone semantics change.
fn step_flying(pos: &mut Position, target: Position, arrival_dist: i32) -> bool {
    if pos.manhattan_distance(&target) <= arrival_dist {
        return true;
    }
    let dx = (target.x() - pos.x()).signum();
    let dy = (target.y() - pos.y()).signum();
    pos.set_tile(pos.x() + dx, pos.y() + dy);
    pos.manhattan_distance(&target) <= arrival_dist
}

/// Nearest map-edge position from `pos`. Used by `resolve_flee_sky` so
/// the hawk heads off-map.
fn nearest_edge_target(pos: Position, map_width: i32, map_height: i32) -> Position {
    let d_left = pos.x();
    let d_right = map_width - 1 - pos.x();
    let d_top = pos.y();
    let d_bottom = map_height - 1 - pos.y();
    let min = d_left.min(d_right).min(d_top).min(d_bottom);
    if min == d_left {
        Position::new(0, pos.y())
    } else if min == d_right {
        Position::new(map_width - 1, pos.y())
    } else if min == d_top {
        Position::new(pos.x(), 0)
    } else {
        Position::new(pos.x(), map_height - 1)
    }
}

// ---------------------------------------------------------------------------
// Resolver: SoarTo
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `SoarTo` (hawk)
///
/// **Real-world effect** — moves the hawk one tile toward the pre-resolved
/// target position for its planned `HawkZone`. Position mutates each tick
/// until arrival; on arrival the step advances.
///
/// **Plan-level preconditions** — emitted by `HawkDomain` for every
/// `SoarTo(zone)` action; target position is materialized from the
/// abstract zone at plan-build time and written to
/// `step_state.target_position`.
///
/// **Runtime preconditions** — `step_state.target_position` MUST be
/// `Some`; the step Fails otherwise. A 200-tick watchdog also fails the
/// step if no progress is made (defensive against pathological target
/// resolution).
///
/// **Witness** — `StepOutcome<()>`. SoarTo is pure movement: the
/// real-world effect (position change) occurs unconditionally once the
/// precondition holds, so no Feature is gated on this resolver.
///
/// **Feature emission** — none.
pub fn resolve_soar_to(
    pos: &mut Position,
    step_state: &mut HawkStepState,
    _map: &TileMap,
) -> StepOutcome<()> {
    let Some(target) = step_state.target_position else {
        return StepOutcome::bare(StepResult::Fail("no target position for SoarTo".into()));
    };
    if step_flying(pos, target, 1) {
        return StepOutcome::bare(StepResult::Advance);
    }
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed > 200 {
        return StepOutcome::bare(StepResult::Fail("soar timeout".into()));
    }
    StepOutcome::bare(StepResult::Continue)
}

// ---------------------------------------------------------------------------
// Resolver: SpotPrey
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `SpotPrey` (hawk)
///
/// **Real-world effect** — checks whether any prey position falls
/// within `detection_range` of the hawk. The witness records whether
/// the spotting *occurred* this call.
///
/// **Plan-level preconditions** — emitted by `HawkDomain` under
/// `HungerOk(false)` (hungry hawk) and `ZoneIs(Sky | HuntingGround)`.
///
/// **Runtime preconditions** — none beyond the prey snapshot the caller
/// supplies. A 100-tick watchdog fails the step if the hawk doesn't
/// spot any prey (caller can replan to a different hunting ground).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff at least one prey
/// position was within detection range this call.
///
/// **Feature emission** — caller passes `Feature::HawkSpottedPrey`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_spot_prey(
    pos: &Position,
    prey_positions: &[Position],
    step_state: &mut HawkStepState,
    detection_range: i32,
) -> StepOutcome<bool> {
    let spotted = prey_positions
        .iter()
        .any(|p| p.manhattan_distance(pos) <= detection_range);
    if spotted {
        return StepOutcome::witnessed(StepResult::Advance);
    }
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed > 100 {
        return StepOutcome::unwitnessed(StepResult::Fail("no prey spotted".into()));
    }
    StepOutcome::unwitnessed(StepResult::Continue)
}

// ---------------------------------------------------------------------------
// Resolver: DiveAttack
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `DiveAttack` (hawk)
///
/// **Real-world effect** — moves the hawk toward the nearest known prey
/// position and, on arrival inside `strike_range`, witnesses the dive
/// with the prey entity. Kill-attribution is performed by
/// `predator_hunt_prey` (per ticket 025 §12) — this resolver records
/// the *dive event*, not the kill.
///
/// **Plan-level preconditions** — emitted after `SpotPrey` succeeds;
/// `HawkDomain` requires `PreySpotted(true)`.
///
/// **Runtime preconditions** — the `prey` slice must be non-empty;
/// Fails otherwise. A 60-tick watchdog fails the dive if the hawk
/// can't close to `strike_range` in time (prey moved / escaped).
///
/// **Witness** — `StepOutcome<Option<Entity>>`. `Some(entity)` iff the
/// hawk arrived in `strike_range` of `entity` this call.
///
/// **Feature emission** — caller passes `Feature::HawkDiveLanded`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_dive_attack(
    pos: &mut Position,
    step_state: &mut HawkStepState,
    prey: &[(Entity, Position)],
    strike_range: i32,
) -> StepOutcome<Option<Entity>> {
    let Some((target_entity, target_pos)) = prey
        .iter()
        .min_by_key(|(_, p)| p.manhattan_distance(pos))
        .copied()
    else {
        return StepOutcome::unwitnessed(StepResult::Fail("no prey for dive".into()));
    };
    if step_flying(pos, target_pos, strike_range) {
        return StepOutcome::witnessed_with(StepResult::Advance, target_entity);
    }
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed > 60 {
        return StepOutcome::unwitnessed(StepResult::Fail("dive timeout".into()));
    }
    StepOutcome::unwitnessed(StepResult::Continue)
}

// ---------------------------------------------------------------------------
// Resolver: Rest
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `Rest` (hawk)
///
/// **Real-world effect** — pure-duration step. The witness fires on the
/// tick the rest completes (i.e. once `ticks_elapsed >= ticks_to_rest`)
/// so the caller knows when to refresh `HawkState.last_perch_tick`.
///
/// **Plan-level preconditions** — emitted by `HawkDomain` for Resting
/// plans (`ZoneIs(Perch)`).
///
/// **Runtime preconditions** — none; time-gated.
///
/// **Witness** — `StepOutcome<bool>`. `true` on the tick the rest
/// duration completes.
///
/// **Feature emission** — caller passes `Feature::HawkPerched`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_rest(step_state: &mut HawkStepState, ticks_to_rest: u64) -> StepOutcome<bool> {
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed >= ticks_to_rest {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}

// ---------------------------------------------------------------------------
// Resolver: FleeSky
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `FleeSky` (hawk)
///
/// **Real-world effect** — moves the hawk one tile toward the nearest
/// map edge each tick. The witness fires on the tick the hawk reaches
/// the edge band (within 2 tiles).
///
/// **Plan-level preconditions** — emitted by `HawkDomain` for Fleeing
/// plans; selected when health is low or cats are nearby.
///
/// **Runtime preconditions** — none. A 200-tick watchdog fails the
/// step if the hawk can't reach the edge in time.
///
/// **Witness** — `StepOutcome<bool>`. `true` on the tick the hawk
/// reaches the edge band.
///
/// **Feature emission** — caller passes `Feature::HawkFled` (Positive)
/// to `record_if_witnessed`.
pub fn resolve_flee_sky(
    pos: &mut Position,
    step_state: &mut HawkStepState,
    map: &TileMap,
) -> StepOutcome<bool> {
    let target = nearest_edge_target(*pos, map.width, map.height);
    if step_flying(pos, target, 2) {
        return StepOutcome::witnessed(StepResult::Advance);
    }
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed > 200 {
        return StepOutcome::unwitnessed(StepResult::Fail("flee timeout".into()));
    }
    StepOutcome::unwitnessed(StepResult::Continue)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::Terrain;

    #[test]
    fn soar_advances_at_target() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(5, 5);
        let mut state = HawkStepState {
            target_position: Some(Position::new(5, 5)),
            ..HawkStepState::default()
        };
        let outcome = resolve_soar_to(&mut pos, &mut state, &map);
        assert!(matches!(outcome.result, StepResult::Advance));
    }

    #[test]
    fn soar_fails_without_target() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(5, 5);
        let mut state = HawkStepState::default();
        let outcome = resolve_soar_to(&mut pos, &mut state, &map);
        assert!(matches!(outcome.result, StepResult::Fail(_)));
    }

    #[test]
    fn soar_moves_diagonally_then_advances() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(0, 0);
        let mut state = HawkStepState {
            target_position: Some(Position::new(2, 2)),
            ..HawkStepState::default()
        };
        // Tick 1 moves to (1,1) — still 2 away (Manhattan).
        let o1 = resolve_soar_to(&mut pos, &mut state, &map);
        assert!(matches!(o1.result, StepResult::Continue));
        assert_eq!(pos, Position::new(1, 1));
        // Tick 2 moves to (2,2) — arrived.
        let o2 = resolve_soar_to(&mut pos, &mut state, &map);
        assert!(matches!(o2.result, StepResult::Advance));
    }

    #[test]
    fn spot_prey_witnesses_when_in_range() {
        let pos = Position::new(5, 5);
        let prey = vec![Position::new(8, 5)];
        let mut state = HawkStepState::default();
        let outcome = resolve_spot_prey(&pos, &prey, &mut state, 5);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness);
    }

    #[test]
    fn spot_prey_unwitnessed_when_out_of_range() {
        let pos = Position::new(5, 5);
        let prey = vec![Position::new(20, 20)];
        let mut state = HawkStepState::default();
        let outcome = resolve_spot_prey(&pos, &prey, &mut state, 5);
        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(!outcome.witness);
    }

    #[test]
    fn dive_attack_witnesses_when_arrived() {
        let mut pos = Position::new(8, 5);
        let prey = vec![(Entity::from_bits(7), Position::new(8, 6))];
        let mut state = HawkStepState::default();
        let outcome = resolve_dive_attack(&mut pos, &mut state, &prey, 1);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert_eq!(outcome.witness, Some(Entity::from_bits(7)));
    }

    #[test]
    fn dive_attack_fails_without_prey() {
        let mut pos = Position::new(5, 5);
        let mut state = HawkStepState::default();
        let outcome = resolve_dive_attack(&mut pos, &mut state, &[], 2);
        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(outcome.witness.is_none());
    }

    #[test]
    fn rest_advances_after_duration_and_witnesses() {
        let mut state = HawkStepState::default();
        let o1 = resolve_rest(&mut state, 3);
        assert!(matches!(o1.result, StepResult::Continue));
        assert!(!o1.witness);
        let o2 = resolve_rest(&mut state, 3);
        assert!(matches!(o2.result, StepResult::Continue));
        let o3 = resolve_rest(&mut state, 3);
        assert!(matches!(o3.result, StepResult::Advance));
        assert!(o3.witness);
    }

    #[test]
    fn flee_sky_witnesses_at_edge() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(1, 10);
        let mut state = HawkStepState::default();
        let outcome = resolve_flee_sky(&mut pos, &mut state, &map);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness);
    }
}
