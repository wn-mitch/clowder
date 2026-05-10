use bevy_ecs::prelude::*;

/// Spatial grid tracking cat patrol presence as a deterrent gradient
/// for foxes (ticket 256 R5).
///
/// Symmetric counterpart to `FoxScentMap`: cats deposit when their
/// `current_action == Action::Patrol`; foxes read this map as routing
/// cost via `CatPatrolDeterrentOverlay`. The deterrent decays globally
/// each tick so passing patrols don't permanently lock foxes out of
/// corridors — only sustained patrol presence creates a meaningful
/// detour for foxes.
///
/// Distinct from `CatPresenceMap`: the presence map deposits
/// unconditionally from any active cat, including idle / foraging /
/// sleeping. The deterrent map is patrol-only — sleeping cats are
/// vulnerable, not threatening.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatPatrolDeterrentMap {
    /// Flat row-major grid of deterrent intensity (0.0–1.0).
    pub marks: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl CatPatrolDeterrentMap {
    /// Build a deterrent grid for a map of `map_w × map_h` tiles.
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

    /// Get the deterrent intensity at a world position.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.marks[i])
            .unwrap_or(0.0)
    }

    /// Deposit at a world position, clamped to 1.0.
    pub fn deposit(&mut self, x: i32, y: i32, amount: f32) {
        if let Some(i) = self.bucket_index(x, y) {
            self.marks[i] = (self.marks[i] + amount).min(1.0);
        }
    }

    /// Decay all marks by a fixed amount per tick.
    pub fn decay_all(&mut self, decay: f32) {
        for v in &mut self.marks {
            *v = (*v - decay).max(0.0);
        }
    }
}

impl Default for CatPatrolDeterrentMap {
    fn default() -> Self {
        Self::default_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_and_decay() {
        let mut map = CatPatrolDeterrentMap::new(20, 20, 5);
        map.deposit(3, 3, 0.5);
        assert!((map.get(3, 3) - 0.5).abs() < f32::EPSILON);

        map.decay_all(0.1);
        assert!((map.get(3, 3) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn deposit_clamps_to_one() {
        let mut map = CatPatrolDeterrentMap::new(20, 20, 5);
        map.deposit(0, 0, 0.8);
        map.deposit(0, 0, 0.5);
        assert!((map.get(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn decay_floors_at_zero() {
        let mut map = CatPatrolDeterrentMap::new(20, 20, 5);
        map.deposit(0, 0, 0.1);
        map.decay_all(0.5);
        assert_eq!(map.get(0, 0), 0.0);
    }

    #[test]
    fn out_of_bounds_returns_zero() {
        let map = CatPatrolDeterrentMap::new(20, 20, 5);
        assert_eq!(map.get(-1, 5), 0.0);
        assert_eq!(map.get(100, 5), 0.0);
    }
}
