use std::collections::{HashMap, HashSet};

use crate::ai::pathfinding::{find_free_adjacent, find_path};
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::StepResult;

pub fn resolve_move_to(
    pos: &mut Position,
    target_position: Option<Position>,
    cached_path: &mut Option<Vec<Position>>,
    map: &TileMap,
    cat_tile_counts: &HashMap<Position, u32>,
) -> StepResult {
    let Some(target) = target_position else {
        return StepResult::Fail("no target position for MoveTo".into());
    };
    if pos.manhattan_distance(&target) == 0 {
        // Arrival jitter: if another cat is already on this tile, shift to a
        // free neighbor so cats don't stack at destinations.
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
        return StepResult::Advance;
    }
    if cached_path.is_none() {
        match find_path(*pos, target, map) {
            Some(path) => *cached_path = Some(path),
            None => return StepResult::Fail("no path to target".into()),
        }
    }
    if let Some(ref mut path) = cached_path {
        if path.is_empty() {
            StepResult::Advance
        } else {
            *pos = path.remove(0);
            StepResult::Continue
        }
    } else {
        StepResult::Continue
    }
}
