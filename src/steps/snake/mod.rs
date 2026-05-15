//! Snake step resolvers — execute individual planned snake actions.
//!
//! Each resolver follows the §"GOAP Step Resolver Contract" in CLAUDE.md
//! and returns a [`StepOutcome<W>`]. Positive `Feature::*` emission MUST
//! go through `record_if_witnessed` at the call site.
//!
//! Snakes move on land; movement here uses the same diagonal step shape
//! as the hawk-side flight helper but is still semantically "slithering"
//! — the resolver doesn't apply terrain costs because pathfinding for
//! short snake distances is dominated by raw Manhattan and the GOAP
//! system gates strike on adjacency rather than path length.

use bevy_ecs::prelude::Entity;

use crate::components::physical::Position;
use crate::components::snake_goap_plan::SnakeStepState;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

// ---------------------------------------------------------------------------
// Ground-step helper
// ---------------------------------------------------------------------------

/// Diagonal step toward `target`. Returns `true` once within `arrival_dist`.
fn step_slithering(pos: &mut Position, target: Position, arrival_dist: i32) -> bool {
    if pos.manhattan_distance(&target) <= arrival_dist {
        return true;
    }
    let dx = (target.x - pos.x).signum();
    let dy = (target.y - pos.y).signum();
    pos.x += dx;
    pos.y += dy;
    pos.manhattan_distance(&target) <= arrival_dist
}

fn nearest_edge_target(pos: Position, map_width: i32, map_height: i32) -> Position {
    let d_left = pos.x;
    let d_right = map_width - 1 - pos.x;
    let d_top = pos.y;
    let d_bottom = map_height - 1 - pos.y;
    let min = d_left.min(d_right).min(d_top).min(d_bottom);
    if min == d_left {
        Position::new(0, pos.y)
    } else if min == d_right {
        Position::new(map_width - 1, pos.y)
    } else if min == d_top {
        Position::new(pos.x, 0)
    } else {
        Position::new(pos.x, map_height - 1)
    }
}

// ---------------------------------------------------------------------------
// Resolver: SlideTo
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `SlideTo` (snake)
///
/// **Real-world effect** — moves the snake one tile toward the pre-resolved
/// target position for its planned `SnakeZone`.
///
/// **Plan-level preconditions** — emitted by `SnakeDomain` for every
/// `SlideTo(zone)` action; target is materialized at plan-build time.
///
/// **Runtime preconditions** — `step_state.target_position` MUST be `Some`;
/// Fails otherwise. 200-tick watchdog Fails on stalled travel.
///
/// **Witness** — `StepOutcome<()>`. Pure movement; no Feature gate.
///
/// **Feature emission** — none.
pub fn resolve_slide_to(
    pos: &mut Position,
    step_state: &mut SnakeStepState,
    _map: &TileMap,
) -> StepOutcome<()> {
    let Some(target) = step_state.target_position else {
        return StepOutcome::bare(StepResult::Fail("no target position for SlideTo".into()));
    };
    if step_slithering(pos, target, 1) {
        return StepOutcome::bare(StepResult::Advance);
    }
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed > 200 {
        return StepOutcome::bare(StepResult::Fail("slide timeout".into()));
    }
    StepOutcome::bare(StepResult::Continue)
}

// ---------------------------------------------------------------------------
// Resolver: SetAmbush
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `SetAmbush` (snake)
///
/// **Real-world effect** — pure-duration step (the snake coils and waits).
/// The witness fires on the tick the ambush is established (after
/// `ticks_to_settle`), so the caller can refresh
/// `SnakeState.last_strike_tick`-style scoring pressure and emit the
/// Feature.
///
/// **Plan-level preconditions** — emitted by `SnakeDomain` for Ambushing
/// plans (`ZoneIs(Cover)`).
///
/// **Runtime preconditions** — none; time-gated.
///
/// **Witness** — `StepOutcome<bool>`. `true` on the tick the ambush is
/// established.
///
/// **Feature emission** — caller passes `Feature::SnakeAmbushed`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_set_ambush(
    step_state: &mut SnakeStepState,
    ticks_to_settle: u64,
) -> StepOutcome<bool> {
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed >= ticks_to_settle {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}

// ---------------------------------------------------------------------------
// Resolver: Strike
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `Strike` (snake)
///
/// **Real-world effect** — moves the snake toward the nearest prey and,
/// once inside `strike_range`, witnesses the strike with the prey
/// entity. Kill-attribution is performed by `predator_hunt_prey` (per
/// ticket 025 §12).
///
/// **Plan-level preconditions** — emitted after `SetAmbush` succeeds and
/// prey is sensed (planner's `PreyInRange(true)` gate).
///
/// **Runtime preconditions** — `prey` must be non-empty; Fails otherwise.
/// A 30-tick watchdog Fails the strike if the snake can't close in time.
///
/// **Witness** — `StepOutcome<Option<Entity>>`. `Some(entity)` iff the
/// snake reached strike range of `entity` this call.
///
/// **Feature emission** — caller passes `Feature::SnakeStruckPrey`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_strike(
    pos: &mut Position,
    step_state: &mut SnakeStepState,
    prey: &[(Entity, Position)],
    strike_range: i32,
) -> StepOutcome<Option<Entity>> {
    let Some((target_entity, target_pos)) = prey
        .iter()
        .min_by_key(|(_, p)| p.manhattan_distance(pos))
        .copied()
    else {
        return StepOutcome::unwitnessed(StepResult::Fail("no prey for strike".into()));
    };
    if step_slithering(pos, target_pos, strike_range) {
        return StepOutcome::witnessed_with(StepResult::Advance, target_entity);
    }
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed > 30 {
        return StepOutcome::unwitnessed(StepResult::Fail("strike timeout".into()));
    }
    StepOutcome::unwitnessed(StepResult::Continue)
}

// ---------------------------------------------------------------------------
// Resolver: Bask
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `Bask` (snake)
///
/// **Real-world effect** — pure-duration step. Witness fires on the tick
/// basking completes (i.e. once `ticks_elapsed >= ticks_to_bask`) so the
/// caller knows when to reset `SnakeState.warmth` and emit the Feature.
///
/// **Plan-level preconditions** — emitted by `SnakeDomain` for Basking
/// plans (`ZoneIs(BaskingSpot)`).
///
/// **Runtime preconditions** — none; time-gated. The caller is
/// responsible for checking the snake is actually on warm terrain
/// before invoking — the resolver itself trusts the planner's
/// `ZoneIs(BaskingSpot)` precondition.
///
/// **Witness** — `StepOutcome<bool>`. `true` on the tick basking completes.
///
/// **Feature emission** — caller passes `Feature::SnakeBasked` (Positive)
/// to `record_if_witnessed`.
pub fn resolve_bask(step_state: &mut SnakeStepState, ticks_to_bask: u64) -> StepOutcome<bool> {
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed >= ticks_to_bask {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}

// ---------------------------------------------------------------------------
// Resolver: Retreat
// ---------------------------------------------------------------------------

/// # GOAP step resolver: `Retreat` (snake)
///
/// **Real-world effect** — moves the snake toward the nearest map edge
/// each tick. Witness fires on the tick the snake reaches the edge band.
///
/// **Plan-level preconditions** — emitted by `SnakeDomain` for Fleeing
/// plans; selected when health is low or cats are adjacent.
///
/// **Runtime preconditions** — none. A 200-tick watchdog fails the step
/// if the snake can't reach the edge in time.
///
/// **Witness** — `StepOutcome<bool>`. `true` on arrival at the edge band.
///
/// **Feature emission** — caller passes `Feature::SnakeRetreated`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_retreat(
    pos: &mut Position,
    step_state: &mut SnakeStepState,
    map: &TileMap,
) -> StepOutcome<bool> {
    let target = nearest_edge_target(*pos, map.width, map.height);
    if step_slithering(pos, target, 2) {
        return StepOutcome::witnessed(StepResult::Advance);
    }
    step_state.ticks_elapsed += 1;
    if step_state.ticks_elapsed > 200 {
        return StepOutcome::unwitnessed(StepResult::Fail("retreat timeout".into()));
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
    fn slide_advances_at_target() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(5, 5);
        let mut state = SnakeStepState {
            target_position: Some(Position::new(5, 5)),
            ..SnakeStepState::default()
        };
        let outcome = resolve_slide_to(&mut pos, &mut state, &map);
        assert!(matches!(outcome.result, StepResult::Advance));
    }

    #[test]
    fn slide_fails_without_target() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(5, 5);
        let mut state = SnakeStepState::default();
        let outcome = resolve_slide_to(&mut pos, &mut state, &map);
        assert!(matches!(outcome.result, StepResult::Fail(_)));
    }

    #[test]
    fn set_ambush_witnesses_after_duration() {
        let mut state = SnakeStepState::default();
        let o1 = resolve_set_ambush(&mut state, 3);
        assert!(matches!(o1.result, StepResult::Continue));
        assert!(!o1.witness);
        let _ = resolve_set_ambush(&mut state, 3);
        let o3 = resolve_set_ambush(&mut state, 3);
        assert!(matches!(o3.result, StepResult::Advance));
        assert!(o3.witness);
    }

    #[test]
    fn strike_witnesses_in_range() {
        let mut pos = Position::new(5, 5);
        let prey = vec![(Entity::from_bits(11), Position::new(5, 6))];
        let mut state = SnakeStepState::default();
        let outcome = resolve_strike(&mut pos, &mut state, &prey, 1);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert_eq!(outcome.witness, Some(Entity::from_bits(11)));
    }

    #[test]
    fn strike_fails_without_prey() {
        let mut pos = Position::new(5, 5);
        let mut state = SnakeStepState::default();
        let outcome = resolve_strike(&mut pos, &mut state, &[], 1);
        assert!(matches!(outcome.result, StepResult::Fail(_)));
    }

    #[test]
    fn bask_witnesses_when_complete() {
        let mut state = SnakeStepState::default();
        let o1 = resolve_bask(&mut state, 2);
        assert!(matches!(o1.result, StepResult::Continue));
        assert!(!o1.witness);
        let o2 = resolve_bask(&mut state, 2);
        assert!(matches!(o2.result, StepResult::Advance));
        assert!(o2.witness);
    }

    #[test]
    fn retreat_witnesses_at_edge() {
        let map = TileMap::new(20, 20, Terrain::Grass);
        let mut pos = Position::new(1, 10);
        let mut state = SnakeStepState::default();
        let outcome = resolve_retreat(&mut pos, &mut state, &map);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness);
    }
}
