use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

use crate::components::physical::Position;
use crate::resources::map::TileMap;

// ---------------------------------------------------------------------------
// A* pathfinding
// ---------------------------------------------------------------------------

/// Node in the A* open set. Ordered by `f_score` ascending (lowest first)
/// so `BinaryHeap` (a max-heap) pops the best candidate.
#[derive(Debug, Clone, Copy)]
struct Node {
    pos: Position,
    f_score: u32,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.f_score == other.f_score
    }
}
impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse: lower f_score = higher priority.
        other.f_score.cmp(&self.f_score)
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 8-directional neighbor offsets.
const NEIGHBORS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Chebyshev distance — admissible heuristic for 8-directional movement
/// with minimum edge cost 1.
fn heuristic(a: &Position, b: &Position) -> u32 {
    let dx = (a.x - b.x).unsigned_abs();
    let dy = (a.y - b.y).unsigned_abs();
    dx.max(dy)
}

/// Compute an optimal path from `from` to `to` on the tile map using A*.
///
/// Returns a `Vec<Position>` of waypoints **excluding** `from` and ending
/// at `to`. Returns `None` if `to` is unreachable. Returns an empty `Vec`
/// if `from == to`.
///
/// Edge weights come from [`Terrain::movement_cost()`], so cats naturally
/// prefer open terrain (Grass=1) over dense forest (3) or rock (4).
pub fn find_path(from: Position, to: Position, map: &TileMap) -> Option<Vec<Position>> {
    if from == to {
        return Some(Vec::new());
    }
    if !map.in_bounds(to.x, to.y) || !map.get(to.x, to.y).terrain.is_passable() {
        return None;
    }

    let w = map.width as usize;
    let h = map.height as usize;
    let idx = |p: &Position| (p.y as usize) * w + (p.x as usize);

    // g_score: cheapest known cost from `from` to each tile. u32::MAX = unvisited.
    let mut g_score = vec![u32::MAX; w * h];
    // came_from: previous tile on the best path (-1 = start / unset).
    let mut came_from: Vec<i32> = vec![-1; w * h];

    let start_idx = idx(&from);
    g_score[start_idx] = 0;

    let mut open = BinaryHeap::new();
    open.push(Node {
        pos: from,
        f_score: heuristic(&from, &to),
    });

    while let Some(current) = open.pop() {
        if current.pos == to {
            // Reconstruct path.
            let mut path = Vec::new();
            let mut ci = idx(&current.pos);
            while ci != start_idx {
                let x = (ci % w) as i32;
                let y = (ci / w) as i32;
                path.push(Position::new(x, y));
                ci = came_from[ci] as usize;
            }
            path.reverse();
            return Some(path);
        }

        let current_g = g_score[idx(&current.pos)];
        // Skip stale entries (we may push duplicates with worse f_scores).
        if current.f_score > current_g.saturating_add(heuristic(&current.pos, &to)) {
            continue;
        }

        for &(dx, dy) in &NEIGHBORS {
            let nx = current.pos.x + dx;
            let ny = current.pos.y + dy;
            if !map.in_bounds(nx, ny) {
                continue;
            }
            let terrain = map.get(nx, ny).terrain;
            if !terrain.is_passable() {
                continue;
            }
            let neighbor = Position::new(nx, ny);
            let ni = idx(&neighbor);
            let tentative_g = current_g + terrain.movement_cost();
            if tentative_g < g_score[ni] {
                g_score[ni] = tentative_g;
                came_from[ni] = idx(&current.pos) as i32;
                open.push(Node {
                    pos: neighbor,
                    f_score: tentative_g + heuristic(&neighbor, &to),
                });
            }
        }
    }

    None // No path exists.
}

// ---------------------------------------------------------------------------
// Greedy step-toward pathfinding
// ---------------------------------------------------------------------------

/// Move one tile closer to `to` using greedy directional preference.
///
/// Tries, in order:
/// 1. Diagonal step (dx, dy)
/// 2. Horizontal step (dx, 0)
/// 3. Vertical step (0, dy)
///
/// Returns the next [`Position`] on success, or `None` if every candidate is
/// out-of-bounds or impassable (the entity is stuck).
///
/// This is intentionally simple — it is not A* and will get stuck in local
/// minima (e.g. concave obstacles). That is acceptable for Phase 1.
pub fn step_toward(from: &Position, to: &Position, map: &TileMap) -> Option<Position> {
    if from == to {
        return None;
    }

    let dx = (to.x - from.x).signum();
    let dy = (to.y - from.y).signum();

    let candidates = [
        // Diagonal first
        (from.x + dx, from.y + dy),
        // Then cardinal
        (from.x + dx, from.y),
        (from.x, from.y + dy),
    ];

    for (nx, ny) in candidates {
        // Skip degenerate candidates that equal the current position
        // (happens when dx or dy is 0).
        if nx == from.x && ny == from.y {
            continue;
        }
        if map.in_bounds(nx, ny) && map.get(nx, ny).terrain.is_passable() {
            return Some(Position::new(nx, ny));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Anti-stacking: find a free adjacent tile
// ---------------------------------------------------------------------------

/// Find an unoccupied, passable tile at or adjacent to `target`.
///
/// Returns `target` itself if it is passable and unoccupied. Otherwise checks
/// the 8 neighbors, preferring whichever is closest (Chebyshev) to `hint` (the
/// approaching entity's current position) so the detour is minimal.
///
/// Returns `None` only when *all* 9 candidates are occupied or impassable.
pub fn find_free_adjacent(
    target: Position,
    hint: Position,
    map: &TileMap,
    occupied: &HashSet<Position>,
) -> Option<Position> {
    // Fast path: target itself is fine.
    if map.in_bounds(target.x, target.y)
        && map.get(target.x, target.y).terrain.is_passable()
        && !occupied.contains(&target)
    {
        return Some(target);
    }

    // Check 8 neighbors, pick the one closest to hint.
    let mut best: Option<(Position, u32)> = None;
    for &(dx, dy) in &NEIGHBORS {
        let nx = target.x + dx;
        let ny = target.y + dy;
        if !map.in_bounds(nx, ny) {
            continue;
        }
        if !map.get(nx, ny).terrain.is_passable() {
            continue;
        }
        let candidate = Position::new(nx, ny);
        if occupied.contains(&candidate) {
            continue;
        }
        let dist = heuristic(&candidate, &hint); // Chebyshev
        if best.as_ref().map_or(true, |&(_, d)| dist < d) {
            best = Some((candidate, dist));
        }
    }
    best.map(|(p, _)| p)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::{Terrain, TileMap};

    /// Helper: open 20×20 grass map.
    fn open_map() -> TileMap {
        TileMap::new(20, 20, Terrain::Grass)
    }

    /// step_toward on open terrain moves closer to the target.
    #[test]
    fn moves_closer_on_open_terrain() {
        let map = open_map();
        let from = Position::new(0, 0);
        let to = Position::new(5, 5);

        let next = step_toward(&from, &to, &map).expect("should move on open terrain");

        // Must be strictly closer in Manhattan distance
        let before = from.manhattan_distance(&to);
        let after = next.manhattan_distance(&to);
        assert!(
            after < before,
            "next position {next:?} is not closer to {to:?} than {from:?} (before={before}, after={after})"
        );
    }

    /// When diagonal is blocked by water, step_toward falls back to a cardinal direction.
    #[test]
    fn avoids_water_diagonal_tries_cardinal() {
        let mut map = open_map();

        // from=(0,0), to=(3,3)
        // Diagonal candidate is (1,1) — block it with water
        map.set(1, 1, Terrain::Water);

        let from = Position::new(0, 0);
        let to = Position::new(3, 3);

        let next = step_toward(&from, &to, &map).expect("should find a cardinal fallback");

        // Must not be the blocked diagonal
        assert_ne!(
            next,
            Position::new(1, 1),
            "stepped onto water tile at (1,1)"
        );

        // Must still be closer
        let before = from.manhattan_distance(&to);
        let after = next.manhattan_distance(&to);
        assert!(
            after < before,
            "fallback position {next:?} is not closer to {to:?}"
        );
    }

    /// When target is directly north and the vertical step is blocked,
    /// step_toward must return None — not the current position.
    #[test]
    fn returns_none_when_cardinal_blocked_and_axis_aligned() {
        let mut map = open_map();
        // from=(5,5), to=(5,0) — target is directly north (dx=0, dy=-1)
        // Block the only useful candidate (5,4) with water.
        map.set(5, 4, Terrain::Water);

        let from = Position::new(5, 5);
        let to = Position::new(5, 0);

        let result = step_toward(&from, &to, &map);
        assert!(
            result.is_none(),
            "expected None when only vertical candidate is blocked on axis-aligned path, got {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // A* find_path tests
    // -----------------------------------------------------------------------

    #[test]
    fn find_path_open_terrain() {
        let map = open_map();
        let path = find_path(Position::new(0, 0), Position::new(5, 5), &map)
            .expect("path should exist on open terrain");
        assert_eq!(*path.last().unwrap(), Position::new(5, 5));
        // Optimal diagonal path: 5 steps.
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn find_path_same_position() {
        let map = open_map();
        let path = find_path(Position::new(3, 3), Position::new(3, 3), &map)
            .expect("same-position path should return empty vec");
        assert!(path.is_empty());
    }

    #[test]
    fn find_path_around_water_wall() {
        let mut map = open_map();
        // Build a vertical water wall at x=5, from y=0 to y=8.
        // Leave y=9 open as a gap.
        for y in 0..9 {
            map.set(5, y, Terrain::Water);
        }
        let from = Position::new(4, 4);
        let to = Position::new(6, 4);

        let path = find_path(from, to, &map).expect("should route around the wall");
        assert_eq!(*path.last().unwrap(), to);
        // Path must not cross any water tile.
        for p in &path {
            assert_ne!(
                map.get(p.x, p.y).terrain,
                Terrain::Water,
                "path crossed water at ({}, {})",
                p.x,
                p.y
            );
        }
    }

    #[test]
    fn find_path_unreachable() {
        let mut map = open_map();
        // Surround target (10,10) with water on all 8 sides.
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                map.set(10 + dx, 10 + dy, Terrain::Water);
            }
        }
        let result = find_path(Position::new(0, 0), Position::new(10, 10), &map);
        assert!(result.is_none(), "path to surrounded tile should be None");
    }

    #[test]
    fn find_path_prefers_cheap_terrain() {
        let mut map = TileMap::new(10, 5, Terrain::Grass);
        // Fill a direct corridor (y=2) with DenseForest (cost 3 each).
        for x in 1..9 {
            map.set(x, 2, Terrain::DenseForest);
        }
        // There's a grass route above/below (cost 1 each).
        let from = Position::new(0, 2);
        let to = Position::new(9, 2);

        let path = find_path(from, to, &map).expect("path should exist");
        // Count how many DenseForest tiles the path crosses.
        let forest_tiles = path
            .iter()
            .filter(|p| map.get(p.x, p.y).terrain == Terrain::DenseForest)
            .count();
        // The optimal path should mostly avoid the forest corridor.
        assert!(
            forest_tiles <= 2,
            "path crossed {forest_tiles} forest tiles — should prefer the grass detour"
        );
    }

    #[test]
    fn find_path_to_impassable_target_returns_none() {
        let mut map = open_map();
        map.set(10, 10, Terrain::Water);
        let result = find_path(Position::new(0, 0), Position::new(10, 10), &map);
        assert!(result.is_none(), "path to impassable target should be None");
    }

    // -----------------------------------------------------------------------
    // find_free_adjacent tests
    // -----------------------------------------------------------------------

    #[test]
    fn free_adjacent_returns_target_when_unoccupied() {
        let map = open_map();
        let occupied = HashSet::new();
        let result = find_free_adjacent(Position::new(5, 5), Position::new(0, 0), &map, &occupied);
        assert_eq!(result, Some(Position::new(5, 5)));
    }

    #[test]
    fn free_adjacent_jitters_when_target_occupied() {
        let map = open_map();
        let occupied: HashSet<Position> = [Position::new(5, 5)].into();
        let result = find_free_adjacent(Position::new(5, 5), Position::new(4, 5), &map, &occupied);
        let p = result.expect("should find a free neighbor");
        assert_ne!(
            p,
            Position::new(5, 5),
            "should not return the occupied tile"
        );
        // Must be adjacent to target.
        assert!(
            (p.x - 5).abs() <= 1 && (p.y - 5).abs() <= 1,
            "result {p:?} should be adjacent to (5,5)"
        );
    }

    #[test]
    fn free_adjacent_prefers_closer_to_hint() {
        let map = open_map();
        let occupied: HashSet<Position> = [Position::new(5, 5)].into();
        // Hint at (4, 5) — neighbor (4, 5) should be preferred (it's closest).
        let result = find_free_adjacent(Position::new(5, 5), Position::new(4, 5), &map, &occupied);
        assert_eq!(
            result,
            Some(Position::new(4, 5)),
            "should prefer the neighbor closest to hint"
        );
    }

    #[test]
    fn free_adjacent_returns_none_when_all_blocked() {
        let mut map = open_map();
        let target = Position::new(5, 5);
        // Block target + all 8 neighbors with water.
        for dy in -1..=1 {
            for dx in -1..=1 {
                map.set(5 + dx, 5 + dy, Terrain::Water);
            }
        }
        let occupied = HashSet::new();
        let result = find_free_adjacent(target, Position::new(3, 3), &map, &occupied);
        assert!(result.is_none(), "all tiles blocked — should return None");
    }

    #[test]
    fn free_adjacent_skips_impassable_neighbors() {
        let mut map = open_map();
        let target = Position::new(5, 5);
        // Occupy target, make most neighbors impassable.
        let occupied: HashSet<Position> = [target].into();
        for &(dx, dy) in &NEIGHBORS {
            if !(dx == 1 && dy == 0) {
                map.set(5 + dx, 5 + dy, Terrain::Water);
            }
        }
        let result = find_free_adjacent(target, Position::new(3, 3), &map, &occupied);
        assert_eq!(
            result,
            Some(Position::new(6, 5)),
            "only (6,5) should be passable and unoccupied"
        );
    }
}
