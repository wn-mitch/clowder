use bevy_ecs::prelude::*;

/// Spatial grid tracking ward repulsion coverage across the map.
///
/// Mirrors the bucketed-overlay pattern used by `FoxScentMap` and
/// `CatPresenceMap`. Unlike scent maps (cumulative deposit + global
/// decay), ward coverage is a *current* property — it's recomputed
/// each tick from live `Ward` entities. Each ward stamps a radial
/// falloff `ward.strength * (1 - dist/repel_radius)` into nearby
/// buckets; overlapping wards sum (clamped to 1.0).
///
/// Consumers: ward-placement DSEs sample this map to express
/// anti-clustering — high coverage on a candidate tile means a new
/// ward there is redundant. Listed as Absent in §5.6.3 of the AI
/// substrate refactor spec; ticket 045 brings it online.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WardCoverageMap {
    /// Flat row-major grid of coverage intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl WardCoverageMap {
    /// Build a coverage grid for a map of `map_w × map_h` tiles.
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

    /// Get the coverage intensity at a world position.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    /// Zero every bucket. Called at the start of each tick's rebuild.
    pub fn clear(&mut self) {
        for v in &mut self.marks {
            *v = 0.0;
        }
    }

    /// Add coverage at a world position, clamped to 1.0.
    pub fn deposit(&mut self, x: i32, y: i32, amount: f32) {
        if let Some(i) = self.bucket_index(x, y) {
            self.marks[i] = (self.marks[i] + amount).min(1.0);
        }
    }

    /// Stamp a single ward's coverage onto the grid. The ward at
    /// `(wx, wy)` with `strength` and `repel_radius` paints a linear
    /// falloff into every bucket whose center is within the radius.
    /// Existing coverage from earlier wards in the same tick is summed
    /// (clamped to 1.0) so doubly-warded tiles read fully covered.
    pub fn stamp_ward(&mut self, wx: i32, wy: i32, strength: f32, repel_radius: f32) {
        if repel_radius <= 0.0 || strength <= 0.0 {
            return;
        }
        let r = repel_radius.ceil() as i32;
        let bs = self.bucket_size;
        let bx_center = wx / bs;
        let by_center = wy / bs;
        let bucket_radius = r / bs + 1;
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
                let cx = bx * bs + bs / 2;
                let cy = by * bs + bs / 2;
                let dx = (cx - wx) as f32;
                let dy = (cy - wy) as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > repel_radius {
                    continue;
                }
                let falloff = (1.0 - dist / repel_radius).max(0.0);
                let contribution = strength * falloff;
                let idx = uby * self.grid_w + ubx;
                self.marks[idx] = (self.marks[idx] + contribution).min(1.0);
            }
        }
    }
}

impl Default for WardCoverageMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_reads_zero() {
        let map = WardCoverageMap::new(20, 20, 5);
        assert_eq!(map.get(0, 0), 0.0);
        assert_eq!(map.get(10, 10), 0.0);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = WardCoverageMap::new(20, 20, 5);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(100, 5), 0.0);
    }

    #[test]
    fn stamp_paints_falloff_within_radius() {
        let mut map = WardCoverageMap::new(40, 40, 5);
        map.stamp_ward(20, 20, 1.0, 9.0);
        // Bucket center at the ward should read close to full strength.
        let center = map.get(22, 22);
        assert!(center > 0.5, "expected strong coverage at ward, got {center}");
        // Far outside radius should still be zero.
        assert_eq!(map.get(0, 0), 0.0);
    }

    #[test]
    fn overlapping_stamps_clamp_to_one() {
        let mut map = WardCoverageMap::new(40, 40, 5);
        map.stamp_ward(20, 20, 1.0, 9.0);
        map.stamp_ward(20, 20, 1.0, 9.0);
        let v = map.get(22, 22);
        assert!(v <= 1.0, "expected clamp, got {v}");
        assert!(v > 0.5);
    }

    #[test]
    fn clear_zeroes_all_buckets() {
        let mut map = WardCoverageMap::new(40, 40, 5);
        map.stamp_ward(20, 20, 1.0, 9.0);
        map.clear();
        for v in &map.marks {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn zero_strength_is_noop() {
        let mut map = WardCoverageMap::new(40, 40, 5);
        map.stamp_ward(20, 20, 0.0, 9.0);
        assert_eq!(map.get(22, 22), 0.0);
    }

    #[test]
    fn zero_radius_is_noop() {
        let mut map = WardCoverageMap::new(40, 40, 5);
        map.stamp_ward(20, 20, 1.0, 0.0);
        assert_eq!(map.get(22, 22), 0.0);
    }
}
