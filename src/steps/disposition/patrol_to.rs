use std::collections::{HashMap, HashSet};

use crate::ai::pathfinding::{find_free_adjacent, find_path};
use crate::components::physical::{Needs, Position};
use crate::resources::map::TileMap;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

pub fn resolve_patrol_to(
    pos: &mut Position,
    target_position: Option<Position>,
    cached_path: &mut Option<Vec<Position>>,
    needs: &mut Needs,
    map: &TileMap,
    d: &DispositionConstants,
    cat_tile_counts: &HashMap<Position, u32>,
) -> StepResult {
    let Some(target) = target_position else {
        return StepResult::Fail("no patrol target".into());
    };
    if pos.manhattan_distance(&target) == 0 {
        jitter_if_stacked(pos, map, cat_tile_counts);
        needs.safety = (needs.safety + d.patrol_arrival_safety_gain).min(1.0);
        return StepResult::Advance;
    }
    // Compute A* path on first tick; follow it thereafter.
    if cached_path.is_none() {
        match find_path(*pos, target, map) {
            Some(path) => *cached_path = Some(path),
            None => return StepResult::Fail("no path to patrol target".into()),
        }
    }
    if let Some(ref mut path) = cached_path {
        if path.is_empty() {
            jitter_if_stacked(pos, map, cat_tile_counts);
            needs.safety = (needs.safety + d.patrol_arrival_safety_gain).min(1.0);
            StepResult::Advance
        } else {
            *pos = path.remove(0);
            needs.safety = (needs.safety + d.patrol_per_tile_safety_gain).min(1.0);
            StepResult::Continue
        }
    } else {
        StepResult::Continue
    }
}

fn jitter_if_stacked(pos: &mut Position, map: &TileMap, cat_tile_counts: &HashMap<Position, u32>) {
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
}
