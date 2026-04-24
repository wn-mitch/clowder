use bevy_ecs::prelude::*;

/// Colony-wide fog-of-war exploration map. Tracks which tiles have been
/// discovered by any cat. Tiles start at 0.0 (unknown) and are set to 1.0
/// when explored. They decay slowly over time so distant/old discoveries
/// become worth re-visiting.
#[derive(Resource, Debug, Clone)]
pub struct ExplorationMap {
    pub width: usize,
    pub height: usize,
    tiles: Vec<f32>,
}

impl ExplorationMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            tiles: vec![0.0; width * height],
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
}
