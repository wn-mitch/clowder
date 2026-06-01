use bevy_ecs::prelude::*;

/// 256 R3: derive a patrol sector index for a cat at the current
/// tick. The sector advances every `rotation_ticks` ticks; each cat
/// carries a deterministic per-entity offset so the colony's patrol
/// beats spread across sectors instead of clustering on the same one.
///
/// `entity.index()` is dense for live entities (Bevy's slot
/// allocator), so we scramble it with a small odd multiplier before
/// taking the modulo — adjacent entity indices then land in
/// different sectors. The exact constant doesn't matter for
/// correctness, only for spread.
pub fn patrol_sector_id(
    tick: u64,
    entity: Entity,
    sector_grid_w: usize,
    sector_grid_h: usize,
    rotation_ticks: u64,
) -> usize {
    let rotation = rotation_ticks.max(1);
    let total = sector_grid_w.max(1) * sector_grid_h.max(1);
    if total == 0 {
        return 0;
    }
    let cat_offset = (entity.to_bits() as usize).wrapping_mul(0x9E37);
    let advance = (tick / rotation) as usize;
    advance.wrapping_add(cat_offset) % total
}

/// Spatial grid tracking ward repulsion coverage across the map.
///
/// Mirrors the bucketed-overlay pattern used by `FoxScentMap` and
/// `CatScentMap`. Unlike scent maps (cumulative deposit + global
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

    /// Number of sectors when the bucket grid is partitioned into a
    /// `sector_grid_w × sector_grid_h` overlay. Used by the Patrol DSE
    /// (ticket 256) to rotate cats through sectors of the warded
    /// demesne instead of pulling them all to a single fixed perimeter
    /// tile.
    pub fn num_sectors(&self, sector_grid_w: usize, sector_grid_h: usize) -> usize {
        sector_grid_w.max(1) * sector_grid_h.max(1)
    }

    /// Intensity-weighted centroid of the buckets in sector `sector_id`.
    ///
    /// Sectors overlay the bucket grid in row-major order: sector
    /// `(sx, sy)` covers buckets `bx ∈ [sx · grid_w/sgw, (sx+1) · grid_w/sgw)`,
    /// `by ∈ [sy · grid_h/sgh, (sy+1) · grid_h/sgh)`. The bottom and right
    /// edge sectors absorb any remainder when `grid_w` / `grid_h` aren't
    /// evenly divisible. Returns `None` when every bucket in the sector
    /// is empty (e.g., early-game before any ward has been placed); the
    /// caller falls back to the legacy static anchor.
    ///
    /// Ticket 256 R3.
    pub fn sector_centroid(
        &self,
        sector_id: usize,
        sector_grid_w: usize,
        sector_grid_h: usize,
    ) -> Option<crate::components::physical::Position> {
        let sgw = sector_grid_w.max(1);
        let sgh = sector_grid_h.max(1);
        let total = sgw * sgh;
        if total == 0 {
            return None;
        }
        let sid = sector_id % total;
        let sx = sid % sgw;
        let sy = sid / sgw;

        let bx_start = sx * self.grid_w / sgw;
        let by_start = sy * self.grid_h / sgh;
        let bx_end = if sx + 1 == sgw {
            self.grid_w
        } else {
            (sx + 1) * self.grid_w / sgw
        };
        let by_end = if sy + 1 == sgh {
            self.grid_h
        } else {
            (sy + 1) * self.grid_h / sgh
        };

        let bs = self.bucket_size;
        let mut sum_x = 0.0_f32;
        let mut sum_y = 0.0_f32;
        let mut sum_w = 0.0_f32;
        for by in by_start..by_end {
            for bx in bx_start..bx_end {
                let w = self.marks[by * self.grid_w + bx];
                if w <= 0.0 {
                    continue;
                }
                let cx = (bx as i32) * bs + bs / 2;
                let cy = (by as i32) * bs + bs / 2;
                sum_x += cx as f32 * w;
                sum_y += cy as f32 * w;
                sum_w += w;
            }
        }
        if sum_w <= 0.0 {
            return None;
        }
        Some(crate::components::physical::Position::new(
            (sum_x / sum_w).round() as i32,
            (sum_y / sum_w).round() as i32,
        ))
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
        assert!(
            center > 0.5,
            "expected strong coverage at ward, got {center}"
        );
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

    #[test]
    fn sector_centroid_empty_map_returns_none() {
        let map = WardCoverageMap::new(120, 90, 5);
        for sid in 0..map.num_sectors(4, 3) {
            assert_eq!(map.sector_centroid(sid, 4, 3), None);
        }
    }

    #[test]
    fn sector_centroid_single_ward_in_sector() {
        // 120x90 map with bucket_size=5 → 24x18 buckets.
        // 4x3 sectors → each sector covers 6x6 buckets (= 30x30 tiles).
        // Place a ward at the center of sector (1, 1) which spans
        // bucket-x ∈ [6, 12), bucket-y ∈ [6, 12) → tiles ∈ [30, 60).
        let mut map = WardCoverageMap::new(120, 90, 5);
        map.stamp_ward(45, 45, 1.0, 9.0);
        // (sx=1, sy=1) in a 4-wide sector grid → sector_id = 1 + 4.
        let sid = 5;
        let centroid = map.sector_centroid(sid, 4, 3).expect("non-empty");
        assert!(
            centroid.x() >= 30 && centroid.x() < 60,
            "x in sector: {centroid:?}"
        );
        assert!(
            centroid.y() >= 30 && centroid.y() < 60,
            "y in sector: {centroid:?}"
        );
        // Other sectors stay empty.
        for sid in 0..12 {
            if sid != 5 {
                assert_eq!(map.sector_centroid(sid, 4, 3), None, "sid={sid}");
            }
        }
    }

    #[test]
    fn sector_centroid_weighted_average_of_two_wards() {
        // Two wards in the same sector at different positions; the
        // centroid should sit between them, weighted by coverage.
        let mut map = WardCoverageMap::new(120, 90, 5);
        // Sector (0, 0) spans tiles [0, 30) × [0, 30).
        map.stamp_ward(5, 5, 1.0, 7.0);
        map.stamp_ward(25, 25, 1.0, 7.0);
        let centroid = map.sector_centroid(0, 4, 3).expect("non-empty");
        assert!(
            centroid.x() > 5 && centroid.x() < 25,
            "between wards: {centroid:?}"
        );
        assert!(
            centroid.y() > 5 && centroid.y() < 25,
            "between wards: {centroid:?}"
        );
    }

    #[test]
    fn sector_centroid_modulo_normalizes_sector_id() {
        // sector_id wraps via modulo; sector_id = num_sectors should
        // equal sector_id = 0.
        let mut map = WardCoverageMap::new(120, 90, 5);
        map.stamp_ward(10, 10, 1.0, 7.0);
        let total = map.num_sectors(4, 3);
        assert_eq!(
            map.sector_centroid(0, 4, 3),
            map.sector_centroid(total, 4, 3),
            "sector_id wraps modulo"
        );
    }

    #[test]
    fn patrol_sector_id_advances_with_tick() {
        let mut world = World::new();
        let e = world.spawn_empty().id();
        // With rotation = 1000 and total = 12, the advance increments
        // every 1000 ticks. Verify the same cat at tick 0 and tick 1000
        // lands on different sectors.
        let s0 = patrol_sector_id(0, e, 4, 3, 1000);
        let s1 = patrol_sector_id(1000, e, 4, 3, 1000);
        assert_ne!(s0, s1, "sector advances by 1 every rotation");
    }

    #[test]
    fn patrol_sector_id_distinct_cats_distinct_sectors() {
        // Two cats at the same tick should usually land on different
        // sectors thanks to the per-entity offset. We can't guarantee
        // this for every pair (modulo collisions exist), but the
        // 0x9E37 multiplier should spread dense indices well.
        let mut world = World::new();
        let mut sectors = std::collections::HashSet::new();
        for _ in 0..6 {
            let e = world.spawn_empty().id();
            sectors.insert(patrol_sector_id(0, e, 4, 3, 1000));
        }
        // 6 cats in 12 sectors should easily produce > 1 unique sector.
        assert!(sectors.len() > 1, "spread across sectors: {sectors:?}");
    }

    #[test]
    fn patrol_sector_id_zero_rotation_safe() {
        let mut world = World::new();
        let e = world.spawn_empty().id();
        // rotation_ticks=0 should clamp to 1 internally and not panic.
        let _ = patrol_sector_id(0, e, 4, 3, 0);
    }

    #[test]
    fn num_sectors_clamps_zero_to_one() {
        let map = WardCoverageMap::new(40, 40, 5);
        assert_eq!(map.num_sectors(0, 0), 1);
        assert_eq!(map.num_sectors(1, 1), 1);
        assert_eq!(map.num_sectors(4, 3), 12);
    }
}
