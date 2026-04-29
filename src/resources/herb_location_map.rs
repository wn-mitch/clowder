use bevy_ecs::prelude::*;

use crate::components::magic::HerbKind;

/// Number of `HerbKind` variants. Kept as an explicit constant so the
/// per-kind grid array has a compile-time size; the [`kind_index`]
/// helper's `match` is exhaustive, so adding a `HerbKind` variant
/// without bumping this constant is a build-time error rather than a
/// silent slot collision.
pub const HERB_KIND_COUNT: usize = 8;

/// Spatial influence map of harvestable herb density, keyed by
/// [`HerbKind`].
///
/// §5.6.3 row #8 of `docs/systems/ai-substrate-refactor.md` — sight ×
/// neutral. Re-stamped each tick from live `Herb` entities carrying
/// the `Harvestable` marker. Each plant paints a linear-falloff disc
/// of radius `herb_location_sense_range` weighted by its growth stage
/// (`Sprout` → `Blossom` = 0.25 → 1.0); the per-kind grid lets
/// consumers sample "thornbriar density at this tile" vs. "any-herb
/// density" without re-walking the herb entity set.
///
/// Producer + initial consumer landed by ticket 061. The initial
/// consumer is the herbcraft target-taking DSE
/// (`herbcraft_target_dse`) and the `HasHerbsNearby` marker
/// authoring in `update_target_existence_markers` — both read
/// `total()` (sum across kinds, clamped to 1.0).
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HerbLocationMap {
    /// One flat row-major grid per `HerbKind` variant. Indexed via
    /// [`kind_index`].
    pub per_kind: [Vec<f32>; HERB_KIND_COUNT],
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

/// Compile-time-checked herb-kind → array-index mapping. The
/// exhaustive `match` makes adding a new `HerbKind` a build-time
/// failure (forces a `HERB_KIND_COUNT` bump + a new arm).
pub fn kind_index(kind: HerbKind) -> usize {
    match kind {
        HerbKind::HealingMoss => 0,
        HerbKind::Moonpetal => 1,
        HerbKind::Calmroot => 2,
        HerbKind::Thornbriar => 3,
        HerbKind::Dreamroot => 4,
        HerbKind::Catnip => 5,
        HerbKind::Slumbershade => 6,
        HerbKind::OracleOrchid => 7,
    }
}

impl HerbLocationMap {
    /// Build a presence grid for a map of `map_w × map_h` tiles.
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        let cells = grid_w * grid_h;
        let per_kind = std::array::from_fn(|_| vec![0.0; cells]);
        Self {
            per_kind,
            grid_w,
            grid_h,
            bucket_size,
        }
    }

    /// Default grid sized for the standard 120×90 map with 5-tile
    /// buckets — matches the four ticket-006 colony-faction maps.
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

    /// Per-kind density at a world position.
    pub fn get(&self, kind: HerbKind, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.per_kind[kind_index(kind)][i])
            .unwrap_or(0.0)
    }

    /// Aggregate density across all kinds at a world position. Clamped
    /// to 1.0 — used by the `InfluenceMap` `base_sample` impl and by
    /// the `HasHerbsNearby` marker projection.
    pub fn total(&self, x: i32, y: i32) -> f32 {
        let Some(i) = self.bucket_index(x, y) else {
            return 0.0;
        };
        let mut sum = 0.0;
        for grid in &self.per_kind {
            sum += grid[i];
        }
        sum.min(1.0)
    }

    /// Zero every bucket on every kind. Called at the start of each
    /// tick's rebuild.
    pub fn clear(&mut self) {
        for grid in &mut self.per_kind {
            for v in grid.iter_mut() {
                *v = 0.0;
            }
        }
    }

    /// Stamp a single herb's presence onto the chosen kind's grid.
    /// Same linear-falloff disc as the four ticket-006 maps;
    /// overlapping stamps sum and clamp to 1.0 within a single kind.
    pub fn stamp(&mut self, kind: HerbKind, sx: i32, sy: i32, strength: f32, sense_range: f32) {
        if sense_range <= 0.0 || strength <= 0.0 {
            return;
        }
        let r = sense_range.ceil() as i32;
        let bs = self.bucket_size;
        let bx_center = sx / bs;
        let by_center = sy / bs;
        let bucket_radius = r / bs + 1;
        let grid = &mut self.per_kind[kind_index(kind)];
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
                grid[idx] = (grid[idx] + contribution).min(1.0);
            }
        }
    }
}

impl Default for HerbLocationMap {
    fn default() -> Self {
        Self::default_map()
    }
}

/// Strength contribution of a herb at the given growth stage.
/// `Sprout=0.25 / Bud=0.5 / Bloom=0.75 / Blossom=1.0`. Mature blooms
/// stamp a stronger disc, so the consumer DSE prefers riper patches
/// when distance is otherwise tied.
pub fn growth_stage_strength(stage: crate::components::magic::GrowthStage) -> f32 {
    use crate::components::magic::GrowthStage;
    match stage {
        GrowthStage::Sprout => 0.25,
        GrowthStage::Bud => 0.5,
        GrowthStage::Bloom => 0.75,
        GrowthStage::Blossom => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::magic::GrowthStage;

    #[test]
    fn empty_map_reads_zero() {
        let map = HerbLocationMap::new(20, 20, 5);
        assert_eq!(map.get(HerbKind::Thornbriar, 0, 0), 0.0);
        assert_eq!(map.total(10, 10), 0.0);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = HerbLocationMap::new(20, 20, 5);
        assert_eq!(map.get(HerbKind::HealingMoss, -1, 5), 0.0);
        assert_eq!(map.get(HerbKind::HealingMoss, 100, 5), 0.0);
        assert_eq!(map.total(-1, 5), 0.0);
    }

    #[test]
    fn stamp_paints_falloff_within_radius() {
        let mut map = HerbLocationMap::new(40, 40, 5);
        map.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);
        let center = map.get(HerbKind::Thornbriar, 22, 22);
        assert!(
            center > 0.5,
            "expected strong presence at source, got {center}"
        );
        assert_eq!(map.get(HerbKind::Thornbriar, 0, 0), 0.0);
    }

    #[test]
    fn stamping_one_kind_leaves_others_empty() {
        let mut map = HerbLocationMap::new(40, 40, 5);
        map.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);
        assert!(map.get(HerbKind::Thornbriar, 22, 22) > 0.0);
        assert_eq!(map.get(HerbKind::HealingMoss, 22, 22), 0.0);
        assert_eq!(map.get(HerbKind::Calmroot, 22, 22), 0.0);
    }

    #[test]
    fn total_sums_across_kinds_and_clamps() {
        let mut map = HerbLocationMap::new(40, 40, 5);
        map.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);
        map.stamp(HerbKind::HealingMoss, 20, 20, 1.0, 15.0);
        let total = map.total(22, 22);
        assert!(total > 0.5, "expected combined presence, got {total}");
        assert!(total <= 1.0, "expected clamp, got {total}");
    }

    #[test]
    fn overlapping_stamps_per_kind_clamp_to_one() {
        let mut map = HerbLocationMap::new(40, 40, 5);
        map.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);
        map.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);
        let v = map.get(HerbKind::Thornbriar, 22, 22);
        assert!(v <= 1.0, "expected clamp, got {v}");
        assert!(v > 0.5);
    }

    #[test]
    fn clear_zeroes_all_kinds() {
        let mut map = HerbLocationMap::new(40, 40, 5);
        map.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);
        map.stamp(HerbKind::HealingMoss, 10, 10, 1.0, 15.0);
        map.clear();
        for grid in &map.per_kind {
            for v in grid {
                assert_eq!(*v, 0.0);
            }
        }
        assert_eq!(map.total(22, 22), 0.0);
    }

    #[test]
    fn zero_strength_or_radius_is_noop() {
        let mut map = HerbLocationMap::new(40, 40, 5);
        map.stamp(HerbKind::Thornbriar, 20, 20, 0.0, 15.0);
        assert_eq!(map.get(HerbKind::Thornbriar, 22, 22), 0.0);
        map.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 0.0);
        assert_eq!(map.get(HerbKind::Thornbriar, 22, 22), 0.0);
    }

    #[test]
    fn restamp_after_clear_is_deterministic() {
        let mut a = HerbLocationMap::new(40, 40, 5);
        a.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);
        a.clear();
        a.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);

        let mut b = HerbLocationMap::new(40, 40, 5);
        b.stamp(HerbKind::Thornbriar, 20, 20, 1.0, 15.0);

        for (ga, gb) in a.per_kind.iter().zip(b.per_kind.iter()) {
            assert_eq!(ga, gb);
        }
    }

    #[test]
    fn growth_stage_strength_monotone() {
        assert!(growth_stage_strength(GrowthStage::Sprout) < growth_stage_strength(GrowthStage::Bud));
        assert!(growth_stage_strength(GrowthStage::Bud) < growth_stage_strength(GrowthStage::Bloom));
        assert!(growth_stage_strength(GrowthStage::Bloom) < growth_stage_strength(GrowthStage::Blossom));
        assert_eq!(growth_stage_strength(GrowthStage::Blossom), 1.0);
    }
}
