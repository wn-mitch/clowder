use bevy_ecs::prelude::*;

use crate::components::prey::PreyKind;
use crate::components::sensing::SensorySpecies;
use crate::resources::sim_constants::SensoryConstants;

/// Spatial grid tracking one prey species' scent footprint.
///
/// The grid-based sibling of `FoxScentMap`, introduced in Phase 2B of
/// the AI substrate refactor (§5.6.3 row #1). Prey entities deposit
/// scent on the tiles they occupy each tick; cats sample the grid
/// rather than running a point-to-point wind-aware formula against
/// every prey entity.
///
/// **Inner grid type.** Per §5.6.3 row #5 (ticket 062), this is no
/// longer a `Resource` — the engine-visible resource is `PreyScentMaps`
/// (a registry of five `PreyScentMap` values keyed by `PreyKind`).
/// This struct retains its grid arithmetic methods (`deposit`, `decay_all`,
/// `get`, `highest_nearby`) and is consumed via `PerSpeciesScentRef`
/// for `InfluenceMap` surface and through `PreyScentMaps` for runtime
/// mutation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyScentMap {
    /// Flat row-major grid of scent intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl PreyScentMap {
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

    /// Default grid sized for the standard 120x90 map with 3-tile
    /// buckets. Finer than FoxScentMap's 5-tile buckets because prey
    /// are denser and their scent reads need tile-adjacency resolution
    /// for hunt target selection.
    pub fn default_map() -> Self {
        Self::new(120, 90, 3)
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

    /// Find the highest-scent bucket within manhattan `radius` of a
    /// world position. Returns the world-tile center of that bucket,
    /// or `None` if all nearby buckets are zero. Mirrors
    /// `FoxScentMap::highest_nearby` so hunt-target selection can
    /// route to "where is scent strongest" rather than
    /// iterating-entities + filtering.
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

impl Default for PreyScentMap {
    fn default() -> Self {
        Self::default_map()
    }
}

/// Per-`PreyKind` registry of scent maps. Indexed by `kind as usize`.
///
/// Replaces the pre-062 aggregate `PreyScentMap` `Resource` so cats can
/// (a) discriminate which species' scent is on a tile and
/// (b) eventually attenuate per emitter species (Phase 3 readiness hook
/// — see `PerSpeciesScentRef` in `src/systems/influence_map.rs`).
///
/// `get_any` / `highest_nearby_any` fold across all five sub-maps via
/// `f32::max`, preserving the aggregate read semantics of the pre-062
/// path for existing consumers; `highest_nearby_for` and `for_kind`
/// give species-discriminating access for future dietary-specialization
/// consumers.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyScentMaps {
    maps: [PreyScentMap; 5],
}

impl PreyScentMaps {
    /// Build a registry with five identically-sized sub-maps for a
    /// `map_w × map_h` world. Used by tests; setup uses
    /// `default_maps()`.
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        Self {
            maps: [
                PreyScentMap::new(map_w, map_h, bucket_size),
                PreyScentMap::new(map_w, map_h, bucket_size),
                PreyScentMap::new(map_w, map_h, bucket_size),
                PreyScentMap::new(map_w, map_h, bucket_size),
                PreyScentMap::new(map_w, map_h, bucket_size),
            ],
        }
    }

    /// Five `PreyScentMap::default_map()` copies (120×90, bucket_size=3).
    /// Inserted as the engine-visible resource at world setup.
    pub fn default_maps() -> Self {
        Self {
            maps: [
                PreyScentMap::default_map(),
                PreyScentMap::default_map(),
                PreyScentMap::default_map(),
                PreyScentMap::default_map(),
                PreyScentMap::default_map(),
            ],
        }
    }

    pub fn for_kind(&self, kind: PreyKind) -> &PreyScentMap {
        &self.maps[kind as usize]
    }

    pub fn for_kind_mut(&mut self, kind: PreyKind) -> &mut PreyScentMap {
        &mut self.maps[kind as usize]
    }

    /// Aggregate read: max across all five sub-maps at `(x, y)`.
    /// Preserves the pre-062 single-map read semantics for consumers
    /// that don't discriminate species (Hunt / Hunting DSEs today).
    pub fn get_any(&self, x: i32, y: i32) -> f32 {
        self.maps
            .iter()
            .map(|m| m.get(x, y))
            .fold(0.0_f32, f32::max)
    }

    /// Aggregate spatial scan: same neighborhood walk as
    /// `PreyScentMap::highest_nearby`, but the bucket value at each
    /// candidate position is taken as `get_any` (max across species).
    pub fn highest_nearby_any(&self, x: i32, y: i32, radius: i32) -> Option<(i32, i32)> {
        let first = &self.maps[0];
        let mut best_val = 0.0_f32;
        let mut best_pos: Option<(i32, i32)> = None;
        let bx_center = x / first.bucket_size;
        let by_center = y / first.bucket_size;
        let bucket_radius = radius / first.bucket_size + 1;
        for by in (by_center - bucket_radius)..=(by_center + bucket_radius) {
            for bx in (bx_center - bucket_radius)..=(bx_center + bucket_radius) {
                if bx < 0 || by < 0 {
                    continue;
                }
                let ubx = bx as usize;
                let uby = by as usize;
                if ubx >= first.grid_w || uby >= first.grid_h {
                    continue;
                }
                let wx = bx * first.bucket_size + first.bucket_size / 2;
                let wy = by * first.bucket_size + first.bucket_size / 2;
                let dist = (wx - x).abs() + (wy - y).abs();
                if dist > radius {
                    continue;
                }
                let val = self.get_any(wx, wy);
                if val > best_val {
                    best_val = val;
                    best_pos = Some((wx, wy));
                }
            }
        }
        best_pos
    }

    /// Species-discriminating spatial scan. No consumers yet — present
    /// as the dietary-specialization hook for future Hunt-DSE work that
    /// reads "where is *bird* scent strongest" rather than "where is
    /// any prey strongest."
    pub fn highest_nearby_for(
        &self,
        kind: PreyKind,
        x: i32,
        y: i32,
        radius: i32,
    ) -> Option<(i32, i32)> {
        self.for_kind(kind).highest_nearby(x, y, radius)
    }

    pub fn decay_all(&mut self, decay: f32) {
        for m in self.maps.iter_mut() {
            m.decay_all(decay);
        }
    }

    /// Deposit scent at `(x, y)` in the sub-map for `kind`, scaled by
    /// that species' relative scent emission strength
    /// (`profile.scent.base_range / normalizer`). With `normalizer = 6.0`
    /// (Rat's base_range), Rat deposits at 1.0×, Mouse/Fish at ~0.83×,
    /// Rabbit at ~0.67×, Bird at ~0.33× — matching the ecological
    /// profile.
    pub fn deposit_for_kind(
        &mut self,
        kind: PreyKind,
        x: i32,
        y: i32,
        base_amount: f32,
        sensory: &SensoryConstants,
        normalizer: f32,
    ) {
        let base_range = sensory.profile_for(SensorySpecies::Prey(kind)).scent.base_range;
        let emission_scale = (base_range / normalizer.max(f32::EPSILON)).clamp(0.0, 1.0);
        self.for_kind_mut(kind)
            .deposit(x, y, base_amount * emission_scale);
    }
}

impl Default for PreyScentMaps {
    fn default() -> Self {
        Self::default_maps()
    }
}

/// Stable lookup table for L1 trace metadata names. The trace emitter
/// reads each `PerSpeciesScentRef` adapter's `metadata().name`, so
/// renaming here renames the L1 trace key.
pub fn scent_map_name(kind: PreyKind) -> &'static str {
    match kind {
        PreyKind::Mouse => "prey_scent_mouse",
        PreyKind::Rat => "prey_scent_rat",
        PreyKind::Rabbit => "prey_scent_rabbit",
        PreyKind::Fish => "prey_scent_fish",
        PreyKind::Bird => "prey_scent_bird",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_and_decay() {
        let mut map = PreyScentMap::new(30, 30, 3);
        map.deposit(6, 6, 0.5);
        assert!((map.get(6, 6) - 0.5).abs() < f32::EPSILON);
        map.decay_all(0.1);
        assert!((map.get(6, 6) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = PreyScentMap::new(30, 30, 3);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = PreyScentMap::new(30, 30, 3);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(1000, 5), 0.0);
    }

    #[test]
    fn highest_nearby_finds_strongest_bucket() {
        let mut map = PreyScentMap::new(30, 30, 3);
        // Deposit a hotspot at (10, 10) and a weaker spot at (0, 0).
        map.deposit(10, 10, 0.9);
        map.deposit(0, 0, 0.3);
        // Searching from (8, 8) within radius 5 should surface the
        // (10, 10) bucket.
        let best = map.highest_nearby(8, 8, 5);
        assert!(best.is_some());
        let (wx, wy) = best.unwrap();
        // Bucket (3, 3) with bucket_size 3 → center (9, 10) or (10, 10)-ish.
        // Accept anything in the immediate neighborhood.
        assert!((wx - 10).abs() <= 3);
        assert!((wy - 10).abs() <= 3);
    }

    #[test]
    fn highest_nearby_returns_none_when_all_zero() {
        let map = PreyScentMap::new(30, 30, 3);
        assert!(map.highest_nearby(15, 15, 10).is_none());
    }

    // ----------------------------------------------------------------
    // PreyScentMaps registry tests (ticket 062)
    // ----------------------------------------------------------------

    const ALL_KINDS: [PreyKind; 5] = [
        PreyKind::Mouse,
        PreyKind::Rat,
        PreyKind::Rabbit,
        PreyKind::Fish,
        PreyKind::Bird,
    ];

    #[test]
    fn prey_scent_test_registry_indexes_all_kinds() {
        let mut maps = PreyScentMaps::new(30, 30, 3);
        let positions: [(i32, i32); 5] =
            [(0, 0), (6, 0), (0, 6), (12, 0), (0, 12)];
        for (kind, (x, y)) in ALL_KINDS.iter().zip(positions.iter()) {
            maps.for_kind_mut(*kind).deposit(*x, *y, 1.0);
        }
        for (kind, (x, y)) in ALL_KINDS.iter().zip(positions.iter()) {
            assert!(
                maps.for_kind(*kind).get(*x, *y) > 0.0,
                "{:?} should have scent at its own tile",
                kind
            );
            for other in ALL_KINDS.iter().filter(|k| *k != kind) {
                assert_eq!(
                    maps.for_kind(*other).get(*x, *y),
                    0.0,
                    "{:?} sub-map should be 0 at {:?}'s tile {:?}",
                    other,
                    kind,
                    (x, y)
                );
            }
        }
    }

    #[test]
    fn prey_scent_test_get_any_returns_max() {
        let mut maps = PreyScentMaps::new(30, 30, 3);
        maps.for_kind_mut(PreyKind::Mouse).deposit(0, 0, 0.1);
        maps.for_kind_mut(PreyKind::Rat).deposit(0, 0, 0.5);
        maps.for_kind_mut(PreyKind::Rabbit).deposit(0, 0, 0.9);
        // Fish and Bird stay zero at this tile.
        assert!((maps.get_any(0, 0) - 0.9).abs() < f32::EPSILON);
        // Touch fish/bird at a different tile; aggregate at (0,0) is unaffected.
        maps.for_kind_mut(PreyKind::Fish).deposit(15, 15, 0.7);
        maps.for_kind_mut(PreyKind::Bird).deposit(15, 15, 0.2);
        assert!((maps.get_any(0, 0) - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn prey_scent_test_highest_nearby_any() {
        let mut maps = PreyScentMaps::new(30, 30, 3);
        // Tile A (low): Mouse 0.3 at (3, 3). Tile B (high): Fish 0.9 at (15, 15).
        maps.for_kind_mut(PreyKind::Mouse).deposit(3, 3, 0.3);
        maps.for_kind_mut(PreyKind::Fish).deposit(15, 15, 0.9);
        // Search from (9, 9) within radius wide enough to see both.
        let best = maps.highest_nearby_any(9, 9, 20);
        assert!(best.is_some());
        let (wx, wy) = best.unwrap();
        // Bucket center of (15, 15) — bucket_size 3 → center (16, 16).
        assert!((wx - 15).abs() <= 3);
        assert!((wy - 15).abs() <= 3);
    }

    #[test]
    fn prey_scent_test_highest_nearby_for_isolates_species() {
        let mut maps = PreyScentMaps::new(30, 30, 3);
        // Mouse hot at A, Fish hot at B (B is hotter overall).
        maps.for_kind_mut(PreyKind::Mouse).deposit(3, 3, 0.3);
        maps.for_kind_mut(PreyKind::Fish).deposit(15, 15, 0.9);
        // Mouse-discriminating scan from (9, 9) should pick A, not B.
        let best_mouse = maps.highest_nearby_for(PreyKind::Mouse, 9, 9, 20);
        assert!(best_mouse.is_some());
        let (mx, my) = best_mouse.unwrap();
        assert!((mx - 3).abs() <= 3);
        assert!((my - 3).abs() <= 3);
        // Fish-discriminating scan from same origin picks B.
        let best_fish = maps.highest_nearby_for(PreyKind::Fish, 9, 9, 20);
        assert!(best_fish.is_some());
        let (fx, fy) = best_fish.unwrap();
        assert!((fx - 15).abs() <= 3);
        assert!((fy - 15).abs() <= 3);
    }
}
