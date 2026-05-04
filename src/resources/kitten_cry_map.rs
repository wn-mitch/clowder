use bevy_ecs::prelude::*;

/// Spatial influence map of kitten distress cries, gated by hunger
/// and broadcast on the `Hearing` channel.
///
/// §5.6.3 row #13 of `docs/systems/ai-substrate-refactor.md` —
/// originally landed by ticket 006 as a sight-channel "kitten urgency"
/// map producer-only awaiting ticket 052's `SpatialConsideration`
/// cutover. 052 retired the `sample_map` consideration shape ("zero
/// production callers") and the substrate sat dead until ticket 156
/// repurposed it as a Hearing-channel cry broadcast: kittens cry,
/// adults hear.
///
/// Each `KittenDependency` cat with `hunger < kitten_cry_hunger_threshold`
/// paints a linear-falloff disc of `kitten_cry_sense_range` tiles,
/// strength `(threshold - hunger) / threshold` so quiet kittens don't
/// paint and starving kittens paint loudly. Adults near multiple
/// crying kittens see the contributions sum (clamped to 1.0).
/// Re-stamped per tick rather than decayed because kittens move and
/// hunger changes fast.
///
/// Consumer: `update_kitten_cry_perceived` reads the map at each
/// adult's position and writes a per-cat `kitten_cry_perceived`
/// scalar; `CaretakeDse` (`src/ai/dses/caretake.rs`) reads the
/// scalar via `ScalarConsideration`.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KittenCryMap {
    pub marks: Vec<f32>,
    pub grid_w: usize,
    pub grid_h: usize,
    pub bucket_size: i32,
}

impl KittenCryMap {
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

    pub fn default_map() -> Self {
        Self::new(120, 90, 5)
    }

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

    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    pub fn clear(&mut self) {
        for v in &mut self.marks {
            *v = 0.0;
        }
    }

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

impl Default for KittenCryMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_reads_zero() {
        let map = KittenCryMap::new(20, 20, 5);
        assert_eq!(map.get(10, 10), 0.0);
    }

    #[test]
    fn stamp_paints_falloff_within_radius() {
        let mut map = KittenCryMap::new(40, 40, 5);
        map.stamp(20, 20, 1.0, 12.0);
        assert!(map.get(22, 22) > 0.5);
        assert_eq!(map.get(0, 0), 0.0);
    }

    #[test]
    fn weak_cry_paints_weak_signal() {
        let mut map = KittenCryMap::new(40, 40, 5);
        map.stamp(20, 20, 0.3, 12.0);
        let center = map.get(22, 22);
        assert!(center < 0.4);
        assert!(center > 0.0);
    }

    #[test]
    fn multiple_kittens_sum() {
        let mut map = KittenCryMap::new(40, 40, 5);
        map.stamp(20, 20, 0.4, 12.0);
        map.stamp(20, 20, 0.4, 12.0);
        let center = map.get(22, 22);
        // Two 0.4 sources sum (each contributes nearly 0.4 at center) but
        // the result must still clamp to ≤ 1.0.
        assert!(center > 0.4);
        assert!(center <= 1.0);
    }

    #[test]
    fn clear_zeroes_all_buckets() {
        let mut map = KittenCryMap::new(40, 40, 5);
        map.stamp(20, 20, 1.0, 12.0);
        map.clear();
        for v in &map.marks {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = KittenCryMap::new(20, 20, 5);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(100, 5), 0.0);
    }
}
