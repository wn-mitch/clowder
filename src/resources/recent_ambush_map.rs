use bevy_ecs::prelude::*;

/// Colony-shared spatial memory of recent ambush events.
///
/// Each tile carries a 0.0–1.0 intensity that bumps to 1.0 when a
/// predator ambushes a cat there and exponentially decays each tick.
/// 210's closeout showed ambushes cluster spatially (60–70% of attacks
/// land in 2–3 tile zones near colony center) and temporally (3–5
/// hits per cat over ~2–3k ticks before death) — neither
/// `FoxScentMap` nor `colony_tension_recent` anchors on *where
/// ambushes have actually happened*. `RecentAmbushMap` fills that gap
/// as the colony-shared, event-typed-failure substrate.
///
/// Bucketed-grid layout follows the `FoxScentMap` / `PreyScentMap`
/// pattern (120×90 world with 5-tile buckets → 24×18 = 432 cells).
///
/// **Substrate posture (per ticket 219):** ships dormant — no DSE
/// scores against it yet. The value is sampled into `ScoringContext`
/// and emitted via `ctx_scalars` so it shows up in `trace-*.jsonl`,
/// keeping the substrate observable. Future tickets 220
/// (ward-placement) and 221 (caretake-relocate) consume it. Per
/// §12.1 of `docs/systems/ai-substrate-refactor.md`, this joins
/// `RecentDispositionFailures` / `RecentTargetFailures` /
/// `HuntingPriors::record_failed_search` as a temporary typed-failure
/// proxy that folds into the unified
/// `Memory.LocationModel.last_threat` facet when Talk-of-the-Town
/// cluster C3 (ticket 007) lands.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentAmbushMap {
    /// Flat row-major grid of ambush memory intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl RecentAmbushMap {
    /// Build a memory grid for a map of `map_w × map_h` tiles.
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

    /// Get the ambush-memory intensity at a world position.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    /// Record an ambush at a world position, clamped to 1.0.
    pub fn deposit(&mut self, x: i32, y: i32, amount: f32) {
        if let Some(i) = self.bucket_index(x, y) {
            self.marks[i] = (self.marks[i] + amount).min(1.0);
        }
    }

    /// Apply exponential decay to all buckets. After `half_life_ticks`
    /// calls, a freshly-deposited 1.0 reads ≈0.5. Diverges from the
    /// linear-subtract decay used by `FoxScentMap` / `PreyScentMap`:
    /// ambush memory is an event-echo that fades asymptotically, not
    /// a continuously-replenished trail.
    pub fn decay_all(&mut self, half_life_ticks: u32) {
        let factor = 0.5_f32.powf(1.0 / (half_life_ticks as f32).max(1.0));
        for v in &mut self.marks {
            *v *= factor;
        }
    }
}

impl Default for RecentAmbushMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_then_decay_reduces_value() {
        let mut map = RecentAmbushMap::new(20, 20, 5);
        map.deposit(3, 3, 1.0);
        assert!((map.get(3, 3) - 1.0).abs() < f32::EPSILON);
        map.decay_all(1000);
        // Single decay tick: factor = 0.5^(1/1000) ≈ 0.999307...
        assert!(map.get(3, 3) < 1.0);
        assert!(map.get(3, 3) > 0.999);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = RecentAmbushMap::new(20, 20, 5);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = RecentAmbushMap::new(20, 20, 5);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(100, 5), 0.0);
        assert_eq!(map.get(5, -1), 0.0);
        assert_eq!(map.get(5, 100), 0.0);
    }

    #[test]
    fn decay_half_life_holds_after_target_tick_count() {
        let mut map = RecentAmbushMap::new(20, 20, 5);
        map.deposit(7, 7, 1.0);
        let half_life: u32 = 100;
        for _ in 0..half_life {
            map.decay_all(half_life);
        }
        // After exactly `half_life` decay calls, the deposit reads ≈0.5.
        // Floating-point composition over 100 multiplies tolerates a
        // small slack; allow ±1e-4 around 0.5.
        let v = map.get(7, 7);
        assert!((v - 0.5).abs() < 1e-4, "got {v}, expected ≈0.5");
    }

    #[test]
    fn bucket_index_maps_correctly() {
        let map = RecentAmbushMap::new(20, 20, 5);
        // (0,0) and (4,4) should be in bucket (0,0) = index 0.
        assert_eq!(map.bucket_index(0, 0), Some(0));
        assert_eq!(map.bucket_index(4, 4), Some(0));
        // (5,0) should be in bucket (1,0) = index 1.
        assert_eq!(map.bucket_index(5, 0), Some(1));
    }

    #[test]
    fn decay_never_goes_negative() {
        let mut map = RecentAmbushMap::new(20, 20, 5);
        map.deposit(2, 2, 0.1);
        for _ in 0..10_000 {
            map.decay_all(1000);
        }
        assert!(map.get(2, 2) >= 0.0);
        assert!(map.get(2, 2) < 1e-3);
    }
}
