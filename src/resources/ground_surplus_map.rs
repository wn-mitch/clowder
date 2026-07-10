use bevy_ecs::prelude::*;

/// Spatial influence map of **ungathered ground food** — `Item` entities at
/// `ItemLocation::OnGround` whose `kind.is_food()`.
///
/// Distinct from [`FoodLocationMap`](crate::resources::FoodLocationMap), which
/// stamps food *infrastructure* (functional `Stores` / `Kitchen` buildings).
/// This map stamps the *windfall* a cat could gather and cache — the founding
/// scatter of raw/forage food, dropped carcasses, spilled surplus. Re-stamped
/// each tick from live ground-food items; each item paints a linear-falloff
/// disc of `ground_surplus_sense_range` tiles so a reader gets a continuous
/// "there is food to gather near here" gradient rather than a binary flag.
///
/// Producer: `update_ground_surplus_map` (`src/systems/buildings.rs`). Consumer:
/// the `surplus_food` belief facet, authored per-cat in `integrate_beliefs`
/// Pass B by sampling this map near each cat's position (ticket: ethological
/// colony-start surplus-caching drive). Registered in the influence-map
/// registry so its samples surface in `trace-*.jsonl` for soak-trace
/// verification.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroundSurplusMap {
    /// Flat row-major grid of presence intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl GroundSurplusMap {
    /// Build a presence grid for a map of `map_w × map_h` tiles.
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

    /// Default grid sized for the standard 120×90 map with 5-tile buckets.
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

    /// Get the presence intensity at a world position.
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

    /// Stamp a single ground-food source's presence onto the grid. The source
    /// at `(sx, sy)` with `strength` and `sense_range` paints a linear falloff
    /// into every bucket whose center is within the radius. Overlapping
    /// sources sum (clamped to 1.0) so a dense pile of scattered food reads
    /// fully covered.
    pub fn stamp(&mut self, sx: i32, sy: i32, strength: f32, sense_range: f32) {
        if sense_range <= 0.0 || strength <= 0.0 {
            return;
        }
        let r = sense_range.ceil() as i32;
        let bs = self.bucket_size;
        let bx_center = sx / bs;
        let by_center = sy / bs;
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
                let dx = (cx - sx) as f32;
                let dy = (cy - sy) as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > sense_range {
                    continue;
                }
                let falloff = (1.0 - dist / sense_range).max(0.0);
                let contribution = strength * falloff;
                let idx = uby * self.grid_w + ubx;
                self.marks[idx] = (self.marks[idx] + contribution).min(1.0);
            }
        }
    }
}

impl Default for GroundSurplusMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_reads_zero() {
        let map = GroundSurplusMap::new(20, 20, 5);
        assert_eq!(map.get(0, 0), 0.0);
        assert_eq!(map.get(10, 10), 0.0);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = GroundSurplusMap::new(20, 20, 5);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(100, 5), 0.0);
    }

    #[test]
    fn stamp_paints_falloff_within_radius() {
        let mut map = GroundSurplusMap::new(40, 40, 5);
        map.stamp(20, 20, 1.0, 10.0);
        let center = map.get(22, 22);
        assert!(
            center > 0.5,
            "expected strong presence at source, got {center}"
        );
        assert_eq!(map.get(0, 0), 0.0);
    }

    #[test]
    fn overlapping_stamps_clamp_to_one() {
        let mut map = GroundSurplusMap::new(40, 40, 5);
        map.stamp(20, 20, 1.0, 10.0);
        map.stamp(20, 20, 1.0, 10.0);
        let v = map.get(22, 22);
        assert!(v <= 1.0, "expected clamp, got {v}");
        assert!(v > 0.5);
    }

    #[test]
    fn clear_zeroes_all_buckets() {
        let mut map = GroundSurplusMap::new(40, 40, 5);
        map.stamp(20, 20, 1.0, 10.0);
        map.clear();
        for v in &map.marks {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn zero_strength_is_noop() {
        let mut map = GroundSurplusMap::new(40, 40, 5);
        map.stamp(20, 20, 0.0, 10.0);
        assert_eq!(map.get(22, 22), 0.0);
    }

    #[test]
    fn zero_radius_is_noop() {
        let mut map = GroundSurplusMap::new(40, 40, 5);
        map.stamp(20, 20, 1.0, 0.0);
        assert_eq!(map.get(22, 22), 0.0);
    }
}
