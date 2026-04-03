use crate::components::physical::Position;
use crate::resources::map::TileMap;

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
        if map.in_bounds(nx, ny) && map.get(nx, ny).terrain.is_passable() {
            return Some(Position::new(nx, ny));
        }
    }

    None
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
}
