use bevy_ecs::prelude::*;

/// Spatial influence map of "where the colony wants to grow."
///
/// Three orthogonal axes per bucket, each in `[0.0, 1.0]`:
///
/// - **`frontier`** — colony-friendly tile. Stamped from `CatScentMap`
///   (where cats live) plus a halo around existing structures so a
///   single-building peninsula scores above raw wilderness. Positive
///   lift in `compute_building_placement`.
/// - **`crowding`** — too many buildings already touch this tile.
///   Stamped from existing `Structure` positions with a small radial
///   penalty (footprint expanded by ~3 tiles). Negative lift.
/// - **`threat`** — predator territory. Max of `FoxScentMap`,
///   `FoxApproachCorridorMap`, and `TileMap` corruption. Negative
///   lift for non-defensive kinds; positive for `Watchtower` /
///   `WardPost` via per-kind weight tables in placement scoring.
///
/// Bucketed-overlay pattern matches `WardCoverageMap` (ticket 045)
/// and `FoodLocationMap` (ticket 006). Re-stamped each tick from a
/// dedicated populator that runs alongside `update_ward_coverage_map`
/// to share schedule-edge slot rather than introduce a new one
/// (`learning_bevy_schedule_edge_perturbation`).
///
/// Ticket 382: replaces the radius-16 spiral search in
/// `find_building_placement` with an influence-map argmax over the
/// whole map.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColonyDistrictMap {
    /// Colony-friendly lift per bucket (0.0–1.0).
    pub frontier: Vec<f32>,
    /// Building-density penalty per bucket (0.0–1.0).
    pub crowding: Vec<f32>,
    /// Predator-territory penalty per bucket (0.0–1.0).
    pub threat: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl ColonyDistrictMap {
    /// Build a district grid for a map of `map_w × map_h` tiles.
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        let n = grid_w * grid_h;
        Self {
            frontier: vec![0.0; n],
            crowding: vec![0.0; n],
            threat: vec![0.0; n],
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

    /// Read the frontier lift at a world position.
    pub fn frontier_at(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.frontier[i])
            .unwrap_or(0.0)
    }

    /// Read the crowding penalty at a world position.
    pub fn crowding_at(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.crowding[i])
            .unwrap_or(0.0)
    }

    /// Read the threat penalty at a world position.
    pub fn threat_at(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.threat[i])
            .unwrap_or(0.0)
    }

    /// Composite "good place to build" scalar — frontier minus the
    /// two penalties, clamped to `[0, 1]`. Per-kind nuance (e.g.
    /// defensive structures wanting threat) lives in the placement
    /// scorer, not here.
    pub fn composite(&self, x: i32, y: i32) -> f32 {
        (self.frontier_at(x, y) - self.crowding_at(x, y) - self.threat_at(x, y))
            .clamp(0.0, 1.0)
    }

    /// Zero every bucket on every axis. Called at the start of each
    /// tick's rebuild.
    pub fn clear(&mut self) {
        for v in &mut self.frontier {
            *v = 0.0;
        }
        for v in &mut self.crowding {
            *v = 0.0;
        }
        for v in &mut self.threat {
            *v = 0.0;
        }
    }

    /// Stamp a linear-falloff disc of `strength × (1 - dist/radius)`
    /// into the given axis at `(sx, sy)`. Overlapping stamps sum
    /// (clamped to 1.0).
    pub fn stamp(&mut self, axis: DistrictAxis, sx: i32, sy: i32, strength: f32, radius: f32) {
        if radius <= 0.0 || strength <= 0.0 {
            return;
        }
        let r = radius.ceil() as i32;
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
                if dist > radius {
                    continue;
                }
                let falloff = (1.0 - dist / radius).max(0.0);
                let contribution = strength * falloff;
                let idx = uby * self.grid_w + ubx;
                let slot = match axis {
                    DistrictAxis::Frontier => &mut self.frontier[idx],
                    DistrictAxis::Crowding => &mut self.crowding[idx],
                    DistrictAxis::Threat => &mut self.threat[idx],
                };
                *slot = (*slot + contribution).min(1.0);
            }
        }
    }
}

impl Default for ColonyDistrictMap {
    fn default() -> Self {
        Self::default_map()
    }
}

/// Which axis of `ColonyDistrictMap` a stamp deposits into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistrictAxis {
    Frontier,
    Crowding,
    Threat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_reads_zero() {
        let map = ColonyDistrictMap::new(20, 20, 5);
        assert_eq!(map.frontier_at(0, 0), 0.0);
        assert_eq!(map.crowding_at(10, 10), 0.0);
        assert_eq!(map.threat_at(5, 5), 0.0);
        assert_eq!(map.composite(5, 5), 0.0);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = ColonyDistrictMap::new(20, 20, 5);
        assert_eq!(map.frontier_at(-1, 5), 0.0);
        assert_eq!(map.crowding_at(100, 5), 0.0);
        assert_eq!(map.composite(-1, -1), 0.0);
    }

    #[test]
    fn stamp_frontier_paints_falloff_within_radius() {
        let mut map = ColonyDistrictMap::new(40, 40, 5);
        map.stamp(DistrictAxis::Frontier, 20, 20, 1.0, 10.0);
        assert!(map.frontier_at(22, 22) > 0.5);
        assert_eq!(map.frontier_at(0, 0), 0.0);
    }

    #[test]
    fn stamp_axes_are_independent() {
        let mut map = ColonyDistrictMap::new(40, 40, 5);
        map.stamp(DistrictAxis::Frontier, 20, 20, 1.0, 10.0);
        map.stamp(DistrictAxis::Crowding, 20, 20, 0.5, 8.0);
        assert!(map.frontier_at(22, 22) > 0.5);
        assert!(map.crowding_at(22, 22) > 0.0);
        assert_eq!(map.threat_at(22, 22), 0.0);
    }

    #[test]
    fn composite_subtracts_penalties_and_clamps() {
        let mut map = ColonyDistrictMap::new(40, 40, 5);
        map.stamp(DistrictAxis::Frontier, 20, 20, 1.0, 10.0);
        map.stamp(DistrictAxis::Threat, 20, 20, 1.0, 10.0);
        // Frontier ≈ threat → composite ≈ 0 (clamped to non-negative).
        assert!(map.composite(22, 22) < 0.2);
        assert!(map.composite(22, 22) >= 0.0);
    }

    #[test]
    fn overlapping_stamps_clamp_to_one() {
        let mut map = ColonyDistrictMap::new(40, 40, 5);
        map.stamp(DistrictAxis::Frontier, 20, 20, 1.0, 10.0);
        map.stamp(DistrictAxis::Frontier, 20, 20, 1.0, 10.0);
        let v = map.frontier_at(22, 22);
        assert!(v <= 1.0);
        assert!(v > 0.5);
    }

    #[test]
    fn clear_zeroes_all_axes() {
        let mut map = ColonyDistrictMap::new(40, 40, 5);
        map.stamp(DistrictAxis::Frontier, 20, 20, 1.0, 10.0);
        map.stamp(DistrictAxis::Crowding, 20, 20, 1.0, 10.0);
        map.stamp(DistrictAxis::Threat, 20, 20, 1.0, 10.0);
        map.clear();
        for axis in [
            DistrictAxis::Frontier,
            DistrictAxis::Crowding,
            DistrictAxis::Threat,
        ] {
            let v = match axis {
                DistrictAxis::Frontier => map.frontier_at(22, 22),
                DistrictAxis::Crowding => map.crowding_at(22, 22),
                DistrictAxis::Threat => map.threat_at(22, 22),
            };
            assert_eq!(v, 0.0, "axis {axis:?} not cleared");
        }
    }

    #[test]
    fn zero_strength_is_noop() {
        let mut map = ColonyDistrictMap::new(40, 40, 5);
        map.stamp(DistrictAxis::Frontier, 20, 20, 0.0, 10.0);
        assert_eq!(map.frontier_at(22, 22), 0.0);
    }

    #[test]
    fn zero_radius_is_noop() {
        let mut map = ColonyDistrictMap::new(40, 40, 5);
        map.stamp(DistrictAxis::Frontier, 20, 20, 1.0, 0.0);
        assert_eq!(map.frontier_at(22, 22), 0.0);
    }
}
