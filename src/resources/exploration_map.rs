use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// Threshold below which a tile is considered "unexplored" for the
/// frontier-centroid computation. Matches the threshold the
/// `unexplored_fraction_nearby` callers in `Explore` DSE scoring already
/// use, so the centroid is consistent with the gating signal.
pub const FRONTIER_THRESHOLD: f32 = 0.5;

/// Colony-wide fog-of-war exploration map. Tracks which tiles have been
/// discovered by any cat. Tiles start at 0.0 (unknown) and are set to 1.0
/// when explored. They decay slowly over time so distant/old discoveries
/// become worth re-visiting.
///
/// `frontier_centroid` caches the centroid of unexplored cells (those
/// below `FRONTIER_THRESHOLD`) — populated once per tick by
/// `update_exploration_centroid` in `systems/needs.rs`. Read by the
/// `Explore` self-state DSE through `LandmarkAnchor::UnexploredFrontierCentroid`
/// and by fox `Dispersing` through the same anchor; the cache avoids
/// rescanning a 120×90 grid 50× per scoring tick.
#[derive(Resource, Debug, Clone)]
pub struct ExplorationMap {
    pub width: usize,
    pub height: usize,
    tiles: Vec<f32>,
    frontier_centroid: Option<Position>,
}

impl ExplorationMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            tiles: vec![0.0; width * height],
            frontier_centroid: None,
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
            Some(y as usize * self.width + x as usize)
        } else {
            None
        }
    }

    /// Mark a tile as explored and return the discovery value (1.0 - previous).
    /// High return = new discovery, low/zero = already known.
    pub fn explore_tile(&mut self, x: i32, y: i32) -> f32 {
        if let Some(idx) = self.index(x, y) {
            let prev = self.tiles[idx];
            self.tiles[idx] = 1.0;
            1.0 - prev
        } else {
            0.0
        }
    }

    /// Mark all tiles within `radius` of (cx, cy) as explored.
    /// Returns the mean discovery value across the area (for need
    /// bonuses). High return = mostly new territory; low = re-tread.
    pub fn explore_area(&mut self, cx: i32, cy: i32, radius: i32) -> f32 {
        let mut total = 0u32;
        let mut discovery_sum = 0.0f32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if let Some(idx) = self.index(cx + dx, cy + dy) {
                    let prev = self.tiles[idx];
                    self.tiles[idx] = 1.0;
                    discovery_sum += 1.0 - prev;
                    total += 1;
                }
            }
        }
        if total == 0 {
            0.0
        } else {
            discovery_sum / total as f32
        }
    }

    /// Get the current exploration value of a tile.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.index(x, y).map_or(0.0, |idx| self.tiles[idx])
    }

    /// Decay all tiles toward 0 so old discoveries become stale.
    pub fn decay(&mut self, rate: f32) {
        for v in &mut self.tiles {
            *v = (*v - rate).max(0.0);
        }
    }

    /// Fraction of tiles within a radius that are unexplored (< threshold).
    /// Used to gate the explore action score.
    pub fn unexplored_fraction_nearby(&self, cx: i32, cy: i32, radius: i32, threshold: f32) -> f32 {
        let mut total = 0u32;
        let mut unexplored = 0u32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let x = cx + dx;
                let y = cy + dy;
                if let Some(idx) = self.index(x, y) {
                    total += 1;
                    if self.tiles[idx] < threshold {
                        unexplored += 1;
                    }
                }
            }
        }
        if total == 0 {
            0.0
        } else {
            unexplored as f32 / total as f32
        }
    }

    /// Fraction of all tiles that have been explored (>= threshold).
    pub fn explored_fraction(&self, threshold: f32) -> f32 {
        let explored = self.tiles.iter().filter(|&&v| v >= threshold).count();
        explored as f32 / self.tiles.len() as f32
    }

    /// Recompute the cached frontier centroid — the mean tile
    /// position of cells whose exploration value is strictly below
    /// `threshold`. Returns `None` when every tile is at or above the
    /// threshold (fully explored colony). Called once per tick by
    /// `systems/needs.rs::update_exploration_centroid` so per-cat
    /// scoring reads a stable, lag-1-tick cache.
    pub fn recompute_frontier_centroid(&mut self, threshold: f32) {
        let mut sum_x: i64 = 0;
        let mut sum_y: i64 = 0;
        let mut count: u32 = 0;
        for (idx, v) in self.tiles.iter().enumerate() {
            if *v < threshold {
                let x = (idx % self.width) as i64;
                let y = (idx / self.width) as i64;
                sum_x += x;
                sum_y += y;
                count += 1;
            }
        }
        self.frontier_centroid = (count > 0).then(|| {
            Position::new(
                (sum_x / count as i64) as i32,
                (sum_y / count as i64) as i32,
            )
        });
    }

    /// Cached centroid of unexplored cells. Populated by
    /// [`Self::recompute_frontier_centroid`]. `None` until the first
    /// recompute pass, or when the colony is fully explored.
    pub fn frontier_centroid(&self) -> Option<Position> {
        self.frontier_centroid
    }
}

impl Default for ExplorationMap {
    fn default() -> Self {
        Self::new(120, 90)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tile_returns_full_discovery() {
        let mut map = ExplorationMap::new(10, 10);
        let discovery = map.explore_tile(5, 5);
        assert!(
            (discovery - 1.0).abs() < 1e-5,
            "new tile should return 1.0 discovery"
        );
    }

    #[test]
    fn already_explored_returns_zero() {
        let mut map = ExplorationMap::new(10, 10);
        map.explore_tile(5, 5);
        let discovery = map.explore_tile(5, 5);
        assert!(
            discovery.abs() < 1e-5,
            "already-explored tile should return 0.0"
        );
    }

    #[test]
    fn decay_makes_tiles_worth_revisiting() {
        let mut map = ExplorationMap::new(10, 10);
        map.explore_tile(5, 5);
        for _ in 0..100 {
            map.decay(0.005);
        }
        let val = map.get(5, 5);
        assert!(val < 0.6, "tile should have decayed; got {val}");
        let discovery = map.explore_tile(5, 5);
        assert!(
            discovery > 0.4,
            "re-exploring decayed tile should give partial discovery"
        );
    }

    #[test]
    fn unexplored_fraction_starts_at_one() {
        let map = ExplorationMap::new(10, 10);
        let frac = map.unexplored_fraction_nearby(5, 5, 3, 0.5);
        assert!((frac - 1.0).abs() < 1e-5, "all tiles should be unexplored");
    }

    #[test]
    fn exploring_reduces_unexplored_fraction() {
        let mut map = ExplorationMap::new(10, 10);
        // Explore a 3x3 block
        for dx in -1..=1 {
            for dy in -1..=1 {
                map.explore_tile(5 + dx, 5 + dy);
            }
        }
        let frac = map.unexplored_fraction_nearby(5, 5, 1, 0.5);
        assert!(frac.abs() < 1e-5, "all nearby tiles explored; got {frac}");
    }

    #[test]
    fn explore_area_marks_disc() {
        let mut map = ExplorationMap::new(20, 20);
        map.explore_area(10, 10, 2);
        // All tiles in 5×5 disc centered at (10,10) should be explored.
        for dy in -2..=2 {
            for dx in -2..=2 {
                let val = map.get(10 + dx, 10 + dy);
                assert!(
                    (val - 1.0).abs() < 1e-5,
                    "tile ({},{}) should be explored; got {val}",
                    10 + dx,
                    10 + dy
                );
            }
        }
        // A tile outside the radius should be untouched.
        assert!(
            map.get(10 + 3, 10).abs() < 1e-5,
            "tile outside radius should be unexplored"
        );
    }

    #[test]
    fn explore_area_discovery_mean() {
        let mut map = ExplorationMap::new(20, 20);
        // Pre-explore half the disc so discovery is partial.
        for dy in -2..=0 {
            for dx in -2..=2 {
                map.explore_tile(10 + dx, 10 + dy);
            }
        }
        // 5×5 = 25 tiles total. Top 3 rows (15 tiles) already explored,
        // bottom 2 rows (10 tiles) are new. Mean discovery = 10/25 = 0.4.
        let discovery = map.explore_area(10, 10, 2);
        assert!(
            (discovery - 0.4).abs() < 1e-5,
            "expected mean discovery ~0.4; got {discovery}"
        );
    }

    #[test]
    fn frontier_centroid_starts_none_until_recomputed() {
        let map = ExplorationMap::new(10, 10);
        assert!(map.frontier_centroid().is_none());
    }

    #[test]
    fn frontier_centroid_is_geometric_center_when_all_unexplored() {
        let mut map = ExplorationMap::new(10, 10);
        map.recompute_frontier_centroid(FRONTIER_THRESHOLD);
        let centroid = map.frontier_centroid().expect("all tiles unexplored");
        // 10×10 grid, all unexplored — mean of 0..=9 in each axis = 4.5,
        // floored to 4 (integer position).
        assert_eq!(centroid, Position::new(4, 4));
    }

    #[test]
    fn frontier_centroid_is_none_when_all_explored() {
        let mut map = ExplorationMap::new(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                map.explore_tile(x, y);
            }
        }
        map.recompute_frontier_centroid(FRONTIER_THRESHOLD);
        assert!(map.frontier_centroid().is_none());
    }

    #[test]
    fn frontier_centroid_shifts_toward_unexplored_region() {
        // 10×10 grid. Explore the left half (x=0..5); centroid of the
        // unexplored right half should be around x=7, y=4.5.
        let mut map = ExplorationMap::new(10, 10);
        for y in 0..10 {
            for x in 0..5 {
                map.explore_tile(x, y);
            }
        }
        map.recompute_frontier_centroid(FRONTIER_THRESHOLD);
        let centroid = map.frontier_centroid().expect("right half unexplored");
        // Right half is x=5..=9, mean = 7.0; integer floor = 7.
        // y range = 0..=9, mean = 4.5, floor = 4.
        assert_eq!(centroid, Position::new(7, 4));
    }
}
