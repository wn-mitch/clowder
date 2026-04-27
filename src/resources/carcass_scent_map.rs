use bevy_ecs::prelude::*;

/// Spatial grid tracking carcass scent. Sibling of `PreyScentMap` and
/// `FoxScentMap` introduced in Phase 2C of the AI substrate refactor
/// (§5.6.3 row #6). Actionable carcasses (`!c.cleansed || !c.harvested`)
/// deposit scent on their tile each tick; cats sample the grid to detect
/// rotting prey within scent range without per-pair `observer_smells_at`
/// iteration.
///
/// **Decay shape (§5.6.5 #6):** "slow fade" — carcasses persist for days
/// but scent must lose a bit each update so a cleansed-and-harvested
/// carcass's residual scent fades after the entity stops re-depositing.
/// Mirrors `PreyScentMap`'s additive-decay shape; see
/// `WildlifeConstants::carcass_scent_decay_rate` for the per-day knob.
///
/// **Substrate-only landing (Phase 2C):** the trace emitter walks this
/// map but the scoring path in `goap.rs:1133–1145` still uses per-pair
/// `observer_smells_at`. Consumer cutover lands separately so the
/// structural change doesn't entangle with the balance shift.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CarcassScentMap {
    /// Flat row-major grid of scent intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl CarcassScentMap {
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
    /// buckets. Matches `PreyScentMap::default_map` so adjacency-grain
    /// reads carry the same resolution semantics.
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
    /// `PreyScentMap::highest_nearby` so harvest-target selection can
    /// route to "where is scent strongest" rather than iterating all
    /// carcass entities.
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

impl Default for CarcassScentMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_and_decay() {
        let mut map = CarcassScentMap::new(30, 30, 3);
        map.deposit(6, 6, 0.5);
        assert!((map.get(6, 6) - 0.5).abs() < f32::EPSILON);
        map.decay_all(0.1);
        assert!((map.get(6, 6) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = CarcassScentMap::new(30, 30, 3);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = CarcassScentMap::new(30, 30, 3);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(1000, 5), 0.0);
    }

    #[test]
    fn highest_nearby_finds_strongest_bucket() {
        let mut map = CarcassScentMap::new(30, 30, 3);
        map.deposit(10, 10, 0.9);
        map.deposit(0, 0, 0.3);
        let best = map.highest_nearby(8, 8, 5);
        assert!(best.is_some());
        let (wx, wy) = best.unwrap();
        assert!((wx - 10).abs() <= 3);
        assert!((wy - 10).abs() <= 3);
    }

    #[test]
    fn highest_nearby_returns_none_when_all_zero() {
        let map = CarcassScentMap::new(30, 30, 3);
        assert!(map.highest_nearby(15, 15, 10).is_none());
    }

    #[test]
    fn decay_floors_at_zero() {
        let mut map = CarcassScentMap::new(30, 30, 3);
        map.deposit(3, 3, 0.05);
        map.decay_all(0.2);
        assert_eq!(map.get(3, 3), 0.0);
    }
}
