//! Ticket 035 — `GraveAuraMap`: small anti-corruption aura around
//! buried graves. Recomputed each tick from live `Grave` entities,
//! mirroring `WardCoverageMap`'s shape and lifecycle.
//!
//! Foundation contribution: each grave stamps a linear-falloff aura
//! `(1 - dist / radius) * strength` into nearby buckets; overlapping
//! graves sum (clamped to 1.0). The aura value at a tile reads as a
//! "grave-soothed-tile" scalar — downstream consumers (a future
//! corruption-pushback consumer + the rest-target picker's safety
//! axis in follow-on ticket #4) sample it.
//!
//! In 035 itself the aura is *registered* in the `InfluenceMapRegistry`
//! and *populated* per tick (so the never-fired-canary surface and L1
//! trace cover it), but no scoring DSE consumes it yet — the
//! anti-corruption balance pass is parked as follow-on ticket #5.

use bevy_ecs::prelude::*;

/// Spatial grid tracking grave-aura intensity across the map.
///
/// Recomputed each tick from live `Grave` entities. Each grave stamps
/// a linear falloff into nearby buckets; overlapping graves sum
/// (clamped to 1.0). Same bucketed-overlay pattern as
/// `WardCoverageMap` (see `src/resources/ward_coverage_map.rs`).
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraveAuraMap {
    pub marks: Vec<f32>,
    pub grid_w: usize,
    pub grid_h: usize,
    pub bucket_size: i32,
}

impl GraveAuraMap {
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

    /// Stamp a single grave's aura onto the grid.
    pub fn stamp_grave(&mut self, gx: i32, gy: i32, strength: f32, radius: f32) {
        if radius <= 0.0 || strength <= 0.0 {
            return;
        }
        let r = radius.ceil() as i32;
        let bs = self.bucket_size;
        let bx_center = gx / bs;
        let by_center = gy / bs;
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
                let dx = (cx - gx) as f32;
                let dy = (cy - gy) as f32;
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
}

impl Default for GraveAuraMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_reads_zero() {
        let map = GraveAuraMap::new(20, 20, 5);
        assert_eq!(map.get(10, 10), 0.0);
    }

    #[test]
    fn grave_aura_decays_with_distance() {
        let mut map = GraveAuraMap::new(40, 40, 5);
        map.stamp_grave(20, 20, 1.0, 8.0);
        let at_grave = map.get(22, 22);
        let far_inside = map.get(20 + 6, 20);
        let outside = map.get(35, 35);
        assert!(at_grave > far_inside, "near should exceed far-inside");
        assert_eq!(outside, 0.0);
    }

    #[test]
    fn grave_aura_zero_outside_radius() {
        let mut map = GraveAuraMap::new(40, 40, 5);
        map.stamp_grave(20, 20, 1.0, 4.0);
        assert_eq!(map.get(0, 0), 0.0);
    }

    #[test]
    fn overlapping_graves_clamp_to_one() {
        let mut map = GraveAuraMap::new(40, 40, 5);
        map.stamp_grave(20, 20, 1.0, 8.0);
        map.stamp_grave(20, 20, 1.0, 8.0);
        let v = map.get(22, 22);
        assert!(v <= 1.0);
        assert!(v > 0.0);
    }

    #[test]
    fn clear_zeroes_grid() {
        let mut map = GraveAuraMap::new(40, 40, 5);
        map.stamp_grave(20, 20, 1.0, 8.0);
        map.clear();
        for v in &map.marks {
            assert_eq!(*v, 0.0);
        }
    }
}
