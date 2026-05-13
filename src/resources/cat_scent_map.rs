use bevy_ecs::prelude::*;

/// Spatial grid tracking cat territorial scent — the scent residue cats
/// leave on the world.
///
/// Mirrors `FoxScentMap` — same bucketed overlay pattern. Two emission
/// rates: every adult cat deposits a small base amount each tick
/// (steady-state scent presence), and active territorial actions
/// (`Patrol`/`Fight`/`Explore`) add a larger bonus. All buckets decay
/// globally each tick. Foxes read high-scent areas to avoid cat
/// territory, creating the push-pull territorial boundary dynamic;
/// the signpost render overlay surfaces the gradient to the player.
///
/// The map is registered as an `InfluenceMap` on the `Scent` channel
/// with `Faction::Colony`, distinct from `CatPatrolDeterrentMap` (a
/// Sight-channel routing-cost gradient deposited only during
/// `Action::Patrol`).
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatScentMap {
    /// Flat row-major grid of scent intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl CatScentMap {
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

    /// Default grid sized for the standard 120x90 map with 5-tile buckets.
    pub fn default_map() -> Self {
        Self::new(120, 90, 5)
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

    /// Find the highest-scent bucket position within manhattan distance of
    /// a world position. Returns the world-tile center of that bucket.
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

impl Default for CatScentMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_and_decay() {
        let mut map = CatScentMap::new(20, 20, 5);
        map.deposit(3, 3, 0.5);
        assert!((map.get(3, 3) - 0.5).abs() < f32::EPSILON);

        map.decay_all(0.1);
        assert!((map.get(3, 3) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = CatScentMap::new(20, 20, 5);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn base_rate_plateaus_below_patrol_peak() {
        // With base 0.01/tick and decay 0.005/tick, steady state ≈ 2.0
        // (clamped at 1.0). With patrol bonus pushing to 0.10/tick, a
        // patrol tile reaches the 1.0 ceiling in ~10 ticks. The base
        // rate alone is enough to be visible but the patrol bonus
        // dominates the gradient.
        let mut base_tile = CatScentMap::new(20, 20, 5);
        let mut patrol_tile = CatScentMap::new(20, 20, 5);
        for _ in 0..20 {
            base_tile.deposit(0, 0, 0.01);
            base_tile.decay_all(0.005);
            patrol_tile.deposit(0, 0, 0.10);
            patrol_tile.decay_all(0.005);
        }
        // Patrol tile should be at or near the 1.0 ceiling.
        assert!(patrol_tile.get(0, 0) > 0.9);
        // Base-only tile should be visible but well below ceiling.
        let b = base_tile.get(0, 0);
        assert!(b > 0.05 && b < 0.3, "base steady state out of band: {b}");
    }
}
