//! Ticket 470 — per-tile siege-fear intensity broadcast by wards currently
//! being besieged by wildlife.
//!
//! Sibling map to [`WardCoverageMap`](super::ward_coverage_map). Producer:
//! [`crate::systems::magic::update_ward_siege_fear_map`] reads each tick's
//! `WildlifeAiState::EncirclingWard` entries and stamps an intensity at
//! the besieged ward's position. Intensity ramps with siege duration
//! (`siege_fear_ramp_ticks`); falloff radius is `siege_fear_radius`.
//!
//! Consumers (Flee / Wander / Explore / HerbcraftWard / `cover_at`) read
//! the value at a position via the `InfluenceMap` trait and compose it
//! with the perceiver's `spirituality` scalar at the consideration layer
//! — high-spirituality cats perceive siege at low intensity, mundane
//! cats only at high intensity. All consumer weights ship at 0.0 (the
//! 301 byte-identical-at-land precedent from `herbcraft_ward.rs:105-122`)
//! pending follow-on tuning.
//!
//! Spec context: the (26,61) seed-42 deep-soak `logs/tuned-42-01eb555d`
//! showed Heron and Simba both bleed to death while reading `safety=1.00`
//! on a besieged-ward tile. Pre-470 the substrate has no positional
//! siege signal — `WardsUnderSiege` is a colony-bool. This map gives
//! consumers a per-tile, per-perceiver-modulated channel to discount
//! "wards present" with "wards under attack."

use bevy_ecs::prelude::*;

/// Spatial grid tracking siege-fear intensity at besieged-ward positions.
///
/// Bucketed overlay matching [`WardCoverageMap`](super::ward_coverage_map)'s
/// shape — 120×90 default world, 5-tile buckets. The producer recomputes
/// the grid each tick from live `WildlifeAiState::EncirclingWard` data;
/// `clear()` zeros every bucket at the start of the rebuild, then each
/// besieged ward stamps a falloff via [`Self::stamp_siege_at`].
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WardSiegeFearMap {
    /// Flat row-major grid of fear intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl WardSiegeFearMap {
    /// Build a fear grid for a map of `map_w × map_h` tiles.
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

    /// Get the fear intensity at a world position.
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

    /// Stamp a single besieged-ward's fear onto the grid with a linear
    /// falloff. `intensity` should encode "how serious is this siege"
    /// (ramping with duration / attacker count) and `radius` the
    /// perception range. Overlapping sieges sum, clamped to 1.0.
    pub fn stamp_siege_at(&mut self, wx: i32, wy: i32, intensity: f32, radius: f32) {
        if radius <= 0.0 || intensity <= 0.0 {
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
                let contribution = intensity * falloff;
                let idx = uby * self.grid_w + ubx;
                self.marks[idx] = (self.marks[idx] + contribution).min(1.0);
            }
        }
    }
}

impl Default for WardSiegeFearMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_reads_zero() {
        let map = WardSiegeFearMap::new(20, 20, 5);
        assert_eq!(map.get(0, 0), 0.0);
        assert_eq!(map.get(10, 10), 0.0);
    }

    #[test]
    fn stamp_paints_falloff_within_radius() {
        let mut map = WardSiegeFearMap::new(40, 40, 5);
        map.stamp_siege_at(20, 20, 1.0, 9.0);
        let center = map.get(22, 22);
        assert!(center > 0.5, "expected strong fear at ward, got {center}");
        assert_eq!(map.get(0, 0), 0.0);
    }

    #[test]
    fn clear_zeroes_all_buckets() {
        let mut map = WardSiegeFearMap::new(40, 40, 5);
        map.stamp_siege_at(20, 20, 1.0, 9.0);
        map.clear();
        for v in &map.marks {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn zero_intensity_is_noop() {
        let mut map = WardSiegeFearMap::new(40, 40, 5);
        map.stamp_siege_at(20, 20, 0.0, 9.0);
        assert_eq!(map.get(22, 22), 0.0);
    }
}
