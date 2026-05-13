use bevy_ecs::prelude::*;

/// 301: spatial grid carrying coordinator-stamped ward-placement
/// intent.
///
/// Populated by [`compute_ward_placement`] when
/// `ward_placement_semantics == DescendingResidual`: each K-round
/// pick stamps a radial intent contribution at its winning tile, so
/// the field encodes "the colony's spread logic wants a ward here."
/// Consumed at score time by the Path-B `HerbcraftWardDse`
/// (`src/ai/dses/herbcraft_ward.rs`) via a substrate-dormant scalar
/// consideration gated on `ward_intent_dse_weight`: when a cat is
/// standing on (or near) a stamped intent tile and the weight is
/// lifted off 0.0, the DSE scores higher there. The cat's own choice
/// of *whether* to set a ward is unchanged, but its propensity to
/// commit when already on an intent tile is biased upward.
///
/// Bucketed pattern mirrors `WardCoverageMap` / `FoxScentMap`. Unlike
/// `WardCoverageMap` (rebuilt every tick from live `Ward` entities),
/// `WardIntentMap` is a *slowly-decaying accumulator*: stamps land
/// during coordinator wakes (every ~20 ticks), and `decay_all`
/// applies a per-tick decay so an intent fades over ~tens of ticks
/// if no cat plants there.
///
/// **Dormancy invariant.** At default `SimConstants` the populator
/// short-circuits (semantics is `SingleShotArgmax`) and the reader's
/// weight is `0.0`, so the resource is allocated but never written
/// to or read for behavior. Existence is for substrate-trace
/// observability; activation is via the two ticket-301 flags.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WardIntentMap {
    /// Flat row-major grid of intent intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl WardIntentMap {
    /// Build an intent grid for a map of `map_w × map_h` tiles.
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

    /// Default grid sized for the standard 120x90 map with 5-tile
    /// buckets — matches `WardCoverageMap::default_map`.
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

    /// Get the intent intensity at a world position.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    /// Stamp coordinator-intent contribution at a world position.
    /// Mirrors `WardCoverageMap::stamp_ward` — paints a linear falloff
    /// `strength * (1 - dist/radius)` into every bucket whose center
    /// is within `radius` tiles. Overlapping stamps sum (clamped to
    /// 1.0).
    pub fn stamp_intent(&mut self, wx: i32, wy: i32, strength: f32, radius: f32) {
        if radius <= 0.0 || strength <= 0.0 {
            return;
        }
        let r = radius.ceil() as i32;
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
                if dist > radius {
                    continue;
                }
                let falloff = (1.0 - dist / radius).max(0.0);
                let contribution = strength * falloff;
                let idx = uby * self.grid_w + ubx;
                self.marks[idx] = (self.marks[idx] + contribution).min(1.0);
            }
        }
    }

    /// Multiplicative per-tick decay (e.g., `0.98` ⇒ ~35-tick
    /// half-life). Multiplicative rather than subtractive keeps the
    /// peak intensity dynamic-range proportional — a stamp that hit
    /// `0.3` fades to zero on the same schedule as one at `1.0`.
    pub fn decay_all(&mut self, factor: f32) {
        let f = factor.clamp(0.0, 1.0);
        for v in &mut self.marks {
            *v *= f;
        }
    }
}

impl Default for WardIntentMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_reads_zero() {
        let map = WardIntentMap::new(40, 40, 5);
        assert_eq!(map.get(10, 10), 0.0);
    }

    #[test]
    fn stamp_paints_falloff_within_radius() {
        let mut map = WardIntentMap::new(40, 40, 5);
        map.stamp_intent(20, 20, 1.0, 9.0);
        assert!(map.get(22, 22) > 0.5, "expected strong intent at stamp");
        assert_eq!(map.get(0, 0), 0.0, "outside radius reads zero");
    }

    #[test]
    fn overlapping_stamps_clamp_to_one() {
        let mut map = WardIntentMap::new(40, 40, 5);
        map.stamp_intent(20, 20, 1.0, 9.0);
        map.stamp_intent(20, 20, 1.0, 9.0);
        assert!(map.get(22, 22) <= 1.0, "must clamp to 1.0");
    }

    #[test]
    fn decay_shrinks_intent_geometrically() {
        let mut map = WardIntentMap::new(40, 40, 5);
        map.stamp_intent(20, 20, 1.0, 9.0);
        let before = map.get(22, 22);
        map.decay_all(0.5);
        let after = map.get(22, 22);
        assert!(
            (after - before * 0.5).abs() < f32::EPSILON,
            "decay(0.5) should halve every cell; before={before} after={after}"
        );
    }

    #[test]
    fn zero_strength_is_noop() {
        let mut map = WardIntentMap::new(40, 40, 5);
        map.stamp_intent(20, 20, 0.0, 9.0);
        assert_eq!(map.get(22, 22), 0.0);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = WardIntentMap::new(40, 40, 5);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(100, 5), 0.0);
    }
}
