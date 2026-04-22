use bevy_ecs::prelude::*;

/// Spatial grid tracking prey scent. The grid-based sibling of
/// `FoxScentMap`, introduced in Phase 2B of the AI substrate refactor
/// (§5.6.3 row #1). Prey entities deposit scent on the tiles they occupy
/// each tick; cats sample the grid to decide whether prey-scent is
/// present at their position rather than running a point-to-point
/// wind-aware formula against every prey entity.
///
/// **Behavioral change from the pre-Phase-2B path:** detection no longer
/// uses the wind-direction dot-product test in
/// `cat_smells_prey_windaware`. Scent diffuses symmetrically via the
/// deposit pattern (for now — a directional plume under wind is a
/// natural follow-up tuning). Range is carried implicitly in the decay
/// rate + deposit intensity rather than as a per-read distance check.
///
/// One aggregate map covers all prey species (mouse / rat / rabbit /
/// fish / bird). Per-species maps are a follow-up if target-selection
/// needs to discriminate "smelled a mouse vs a rabbit."
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyScentMap {
    /// Flat row-major grid of scent intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl PreyScentMap {
    /// Build a scent grid for a map of `map_w × map_h` tiles.
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        Self {
            marks: vec![0.0; grid_w * grid_h],
            grid_w,
            grid_h,
            bucket_size,
        }
    }

    /// Default grid sized for the standard 120x90 map with 3-tile
    /// buckets. Finer than FoxScentMap's 5-tile buckets because prey
    /// are denser and their scent reads need tile-adjacency resolution
    /// for hunt target selection.
    pub fn default_map() -> Self {
        Self::new(120, 90, 3)
    }

    /// Convert a world position to a flat grid index.
    pub fn bucket_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let bx = (x / self.bucket_size) as usize;
        let by = (y / self.bucket_size) as usize;
        if bx >= self.grid_w || by >= self.grid_h {
            return None;
        }
        Some(by * self.grid_w + bx)
    }

    /// Get the scent intensity at a world position.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    /// Deposit scent at a world position, clamped to 1.0.
    pub fn deposit(&mut self, x: i32, y: i32, amount: f32) {
        if let Some(i) = self.bucket_index(x, y) {
            self.marks[i] = (self.marks[i] + amount).min(1.0);
        }
    }

    /// Decay all scent marks by a fixed amount per tick.
    pub fn decay_all(&mut self, decay: f32) {
        for v in &mut self.marks {
            *v = (*v - decay).max(0.0);
        }
    }

    /// Find the highest-scent bucket within manhattan `radius` of a
    /// world position. Returns the world-tile center of that bucket,
    /// or `None` if all nearby buckets are zero. Mirrors
    /// `FoxScentMap::highest_nearby` so hunt-target selection can
    /// route to "where is scent strongest" rather than
    /// iterating-entities + filtering.
    pub fn highest_nearby(&self, x: i32, y: i32, radius: i32) -> Option<(i32, i32)> {
        let mut best_val = 0.0f32;
        let mut best_pos = None;
        let bx_center = x / self.bucket_size;
        let by_center = y / self.bucket_size;
        let bucket_radius = radius / self.bucket_size + 1;
        for by in (by_center - bucket_radius)..=(by_center + bucket_radius) {
            for bx in (bx_center - bucket_radius)..=(bx_center + bucket_radius) {
                if bx < 0 || by < 0 {
                    continue;
                }
                let ubx = bx as usize;
                let uby = by as usize;
                if ubx >= self.grid_w || uby >= self.grid_h {
                    continue;
                }
                let idx = uby * self.grid_w + ubx;
                let val = self.marks[idx];
                let wx = bx * self.bucket_size + self.bucket_size / 2;
                let wy = by * self.bucket_size + self.bucket_size / 2;
                let dist = (wx - x).abs() + (wy - y).abs();
                if dist <= radius && val > best_val {
                    best_val = val;
                    best_pos = Some((wx, wy));
                }
            }
        }
        best_pos
    }
}

impl Default for PreyScentMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_and_decay() {
        let mut map = PreyScentMap::new(30, 30, 3);
        map.deposit(6, 6, 0.5);
        assert!((map.get(6, 6) - 0.5).abs() < f32::EPSILON);
        map.decay_all(0.1);
        assert!((map.get(6, 6) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = PreyScentMap::new(30, 30, 3);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = PreyScentMap::new(30, 30, 3);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(1000, 5), 0.0);
    }

    #[test]
    fn highest_nearby_finds_strongest_bucket() {
        let mut map = PreyScentMap::new(30, 30, 3);
        // Deposit a hotspot at (10, 10) and a weaker spot at (0, 0).
        map.deposit(10, 10, 0.9);
        map.deposit(0, 0, 0.3);
        // Searching from (8, 8) within radius 5 should surface the
        // (10, 10) bucket.
        let best = map.highest_nearby(8, 8, 5);
        assert!(best.is_some());
        let (wx, wy) = best.unwrap();
        // Bucket (3, 3) with bucket_size 3 → center (9, 10) or (10, 10)-ish.
        // Accept anything in the immediate neighborhood.
        assert!((wx - 10).abs() <= 3);
        assert!((wy - 10).abs() <= 3);
    }

    #[test]
    fn highest_nearby_returns_none_when_all_zero() {
        let map = PreyScentMap::new(30, 30, 3);
        assert!(map.highest_nearby(15, 15, 10).is_none());
    }
}
