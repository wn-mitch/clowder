use bevy_ecs::prelude::*;

/// Colony-shared spatial memory of fox-approach corridors.
///
/// Each tile carries a 0.0–1.0 intensity that bumps when an active
/// (patrolling) `ShadowFox` advances through it and exponentially
/// decays each tick. The signal accumulates on tiles foxes
/// *traverse* on their way to cats — distinct from `FoxScentMap`
/// (territorial mark, decays in days), `RecentAmbushMap` (event-echo
/// at attack site, decays in ~one day), and the inline
/// fox-spawn-vicinity halo (computed from corruption sources, not
/// observed routes).
///
/// 301's first-light soak named the substrate gap:
/// `compute_ward_placement` had no input for *topological
/// criticality* — which tiles foxes actually walk on to reach the
/// colony. `fox_scent` decays too fast; `corruption` lights spawn
/// sources; `cat_value` and `-distance_cost` both pull placement
/// *toward cats*. Without this map, any selection-rule change just
/// moves wards within the cat cluster.
///
/// **Per-tile resolution** (bucket_size = 1) — diverges from sibling
/// maps' 5-tile bucketing. Corridors are linear topological features
/// (a 2-tile-wide isthmus is the canonical test case): bucket=5
/// would alias the corridor signal onto neighboring non-corridor
/// tiles, defeating the architectural intent. Memory cost is ~43KB
/// (120×90 f32) and decay is O(grid) per tick — both negligible at
/// the simulation's scale. The mirror of `RecentAmbushMap`'s shape
/// otherwise: same `deposit` / `decay_all` / `get` / `bucket_index`
/// API, same exponential-decay semantics.
///
/// **Substrate posture (per ticket 312):** ships dormant — the
/// scorer's `ward_fox_approach_corridor_weight` defaults to 0.0, so
/// no behavior changes at land. The FO-1 scenario
/// (`chokepoint_defense_isthmus`) activates it at fixture level
/// (`weight = 0.3`) to assert the isthmus-corked outcome.
/// Three-seed `just hypothesize` validates at `weight = 0.3` per
/// the four-artifact methodology. FO-4 will migrate the signal into
/// the 258 belief layer once 263–270 establishes the belief-DSE
/// consumer surface.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxApproachCorridorMap {
    /// Flat row-major grid of corridor-traffic intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl FoxApproachCorridorMap {
    /// Build a corridor grid for a map of `map_w × map_h` tiles.
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

    /// Default grid sized for the standard 120×90 map with per-tile
    /// (bucket_size = 1) resolution. See struct rustdoc for the
    /// rationale: corridors are linear features that lose meaning
    /// under bucketing aliasing, and the memory cost (~43KB) is
    /// negligible.
    pub fn default_map() -> Self {
        Self::new(120, 90, 1)
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

    /// Get the corridor-traffic intensity at a world position.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    /// Record a patrolling fox passing through a world position,
    /// clamped to 1.0. Saturates after ~20 deposits at the default
    /// `0.05`/step rate — corridor intensity reads "this tile sees
    /// regular fox traffic", not "exactly N foxes passed today".
    pub fn deposit(&mut self, x: i32, y: i32, amount: f32) {
        if let Some(i) = self.bucket_index(x, y) {
            self.marks[i] = (self.marks[i] + amount).min(1.0);
        }
    }

    /// Apply exponential decay to all buckets. Same shape as
    /// `RecentAmbushMap::decay_all`. Corridor decay is slower than
    /// ambush decay (default 20_000 vs 5_000 tick half-life)
    /// because corridors are stable terrain features — fox patrol
    /// routes persist across many ambush events.
    pub fn decay_all(&mut self, half_life_ticks: u32) {
        let factor = 0.5_f32.powf(1.0 / (half_life_ticks as f32).max(1.0));
        for v in &mut self.marks {
            *v *= factor;
        }
    }
}

impl Default for FoxApproachCorridorMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_then_decay_reduces_value() {
        let mut map = FoxApproachCorridorMap::new(20, 20, 5);
        map.deposit(3, 3, 1.0);
        assert!((map.get(3, 3) - 1.0).abs() < f32::EPSILON);
        map.decay_all(1000);
        assert!(map.get(3, 3) < 1.0);
        assert!(map.get(3, 3) > 0.999);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = FoxApproachCorridorMap::new(20, 20, 5);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = FoxApproachCorridorMap::new(20, 20, 5);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(100, 5), 0.0);
        assert_eq!(map.get(5, -1), 0.0);
        assert_eq!(map.get(5, 100), 0.0);
    }

    #[test]
    fn decay_half_life_holds_after_target_tick_count() {
        let mut map = FoxApproachCorridorMap::new(20, 20, 5);
        map.deposit(7, 7, 1.0);
        let half_life: u32 = 100;
        for _ in 0..half_life {
            map.decay_all(half_life);
        }
        let v = map.get(7, 7);
        assert!((v - 0.5).abs() < 1e-4, "got {v}, expected ≈0.5");
    }

    #[test]
    fn decay_never_goes_negative() {
        let mut map = FoxApproachCorridorMap::new(20, 20, 5);
        map.deposit(2, 2, 0.1);
        for _ in 0..10_000 {
            map.decay_all(1000);
        }
        assert!(map.get(2, 2) >= 0.0);
        assert!(map.get(2, 2) < 1e-3);
    }
}
