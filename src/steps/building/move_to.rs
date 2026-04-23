use std::collections::{HashMap, HashSet};

use crate::ai::pathfinding::{find_free_adjacent, find_path};
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `MoveTo`
///
/// **Real-world effect** — paths the actor toward `target_position`
/// one tile per tick; on arrival (and when stacked), jitters to a
/// free neighbor so cats don't pile on destination tiles.
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
    cat_tile_counts: &HashMap<Position, u32>,
) -> StepOutcome<()> {
    let Some(target) = target_position else {
        return StepOutcome::bare(StepResult::Fail("no target position for MoveTo".into()));
    };
    if pos.manhattan_distance(&target) == 0 {
        if cat_tile_counts.get(pos).copied().unwrap_or(0) > 1 {
            let occupied: HashSet<Position> = cat_tile_counts
                .iter()
                .filter(|(_, &count)| count >= 1)
                .map(|(p, _)| *p)
                .collect();
            if let Some(free) = find_free_adjacent(*pos, *pos, map, &occupied) {
                if free != *pos {
                    *pos = free;
                }
            }
        }
        return StepOutcome::bare(StepResult::Advance);
    }
    if cached_path.is_none() {
        match find_path(*pos, target, map) {
            Some(path) => *cached_path = Some(path),
            None => return StepOutcome::bare(StepResult::Fail("no path to target".into())),
        }
    }
    if let Some(ref mut path) = cached_path {
        if path.is_empty() {
            StepOutcome::bare(StepResult::Advance)
        } else {
            *pos = path.remove(0);
            StepOutcome::bare(StepResult::Continue)
        }
    } else {
        StepOutcome::bare(StepResult::Continue)
    }
}
