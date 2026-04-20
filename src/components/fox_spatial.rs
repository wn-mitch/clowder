//! Fox spatial memory — per-fox bayesian belief grids.
//!
//! Follows the same bucketed overlay pattern as [`HuntingPriors`] and
//! [`FoxScentMap`]. Each fox has its own belief grids, enabling emergent
//! individual territory knowledge and risk perception.
//!
//! [`HuntingPriors`]: crate::components::hunting_priors::HuntingPriors
//! [`FoxScentMap`]: crate::resources::fox_scent_map::FoxScentMap

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

/// Default bucket size (tiles) for all fox spatial grids.
/// Matches `FoxScentMap` so beliefs align with scent geography.
pub const FOX_BUCKET_SIZE: i32 = 5;

/// Standard grid size for the 120x90 map with 5-tile buckets.
const STANDARD_MAP_W: usize = 120;
const STANDARD_MAP_H: usize = 90;

// ---------------------------------------------------------------------------
// FoxHuntingBeliefs — per-fox bayesian prey-density grid
// ---------------------------------------------------------------------------

/// Starting belief: equal probability of prey presence.
pub const HUNTING_DEFAULT_PRIOR: f32 = 0.5;
pub const HUNTING_MIN_BELIEF: f32 = 0.05;
pub const HUNTING_MAX_BELIEF: f32 = 0.95;

/// Per-fox spatial belief grid over prey abundance.
///
/// Updated on successful kills (positive evidence) and fruitless searches
/// (negative evidence). Foxes that hunt different areas develop different
/// beliefs, producing emergent specialization.
#[derive(Component, Debug, Clone)]
pub struct FoxHuntingBeliefs {
    pub beliefs: Vec<f32>,
    pub grid_w: usize,
    pub grid_h: usize,
    pub bucket_size: i32,
}

impl FoxHuntingBeliefs {
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        Self {
            beliefs: vec![HUNTING_DEFAULT_PRIOR; grid_w * grid_h],
            grid_w,
            grid_h,
            bucket_size,
        }
    }

    pub fn default_map() -> Self {
        Self::new(STANDARD_MAP_W, STANDARD_MAP_H, FOX_BUCKET_SIZE)
    }

    /// Convert a world position to a flat belief index.
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
            .map(|i| self.beliefs[i])
            .unwrap_or(HUNTING_DEFAULT_PRIOR)
    }

    /// Reinforce belief after a successful hunt.
    pub fn reinforce(&mut self, pos: Position, amount: f32) {
        if let Some(i) = self.bucket_index(pos.x, pos.y) {
            self.beliefs[i] = (self.beliefs[i] + amount).min(HUNTING_MAX_BELIEF);
        }
    }

    /// Weaken belief after an unsuccessful search.
    pub fn decay(&mut self, pos: Position, amount: f32) {
        if let Some(i) = self.bucket_index(pos.x, pos.y) {
            self.beliefs[i] = (self.beliefs[i] - amount).max(HUNTING_MIN_BELIEF);
        }
    }
}

// ---------------------------------------------------------------------------
// FoxThreatMemory — per-fox danger heatmap
// ---------------------------------------------------------------------------

pub const THREAT_MAX: f32 = 1.0;

/// Per-fox spatial memory of dangerous locations.
///
/// Updated when the fox takes damage, encounters cats, or triggers a ward.
/// Decays slowly over time — foxes eventually retry dangerous areas. Bold
/// foxes discount threat memory; cautious foxes weight it heavily.
#[derive(Component, Debug, Clone)]
pub struct FoxThreatMemory {
    pub threats: Vec<f32>,
    pub grid_w: usize,
    pub grid_h: usize,
    pub bucket_size: i32,
}

impl FoxThreatMemory {
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        Self {
            threats: vec![0.0; grid_w * grid_h],
            grid_w,
            grid_h,
            bucket_size,
        }
    }

    pub fn default_map() -> Self {
        Self::new(STANDARD_MAP_W, STANDARD_MAP_H, FOX_BUCKET_SIZE)
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
            .map(|i| self.threats[i])
            .unwrap_or(0.0)
    }

    /// Record a dangerous encounter at this location.
    pub fn record_threat(&mut self, pos: Position, amount: f32) {
        if let Some(i) = self.bucket_index(pos.x, pos.y) {
            self.threats[i] = (self.threats[i] + amount).min(THREAT_MAX);
        }
    }

    /// Global decay — threats fade from memory over time.
    pub fn decay_all(&mut self, rate: f32) {
        for v in &mut self.threats {
            *v = (*v - rate).max(0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// FoxExplorationMap — per-fox visit tracking
// ---------------------------------------------------------------------------

/// Per-fox tile visitation grid.
///
/// Tracks which areas the fox has explored. Juvenile dispersers prefer
/// unexplored areas; resident foxes patrol under-visited parts of territory.
#[derive(Component, Debug, Clone)]
pub struct FoxExplorationMap {
    pub coverage: Vec<f32>,
    pub grid_w: usize,
    pub grid_h: usize,
    pub bucket_size: i32,
}

impl FoxExplorationMap {
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        Self {
            coverage: vec![0.0; grid_w * grid_h],
            grid_w,
            grid_h,
            bucket_size,
        }
    }

    pub fn default_map() -> Self {
        Self::new(STANDARD_MAP_W, STANDARD_MAP_H, FOX_BUCKET_SIZE)
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
            .map(|i| self.coverage[i])
            .unwrap_or(0.0)
    }

    /// Mark the current position as visited.
    pub fn visit(&mut self, pos: Position) {
        if let Some(i) = self.bucket_index(pos.x, pos.y) {
            self.coverage[i] = (self.coverage[i] + 0.05).min(1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunting_beliefs_default_prior() {
        let b = FoxHuntingBeliefs::default_map();
        assert_eq!(b.get(50, 50), HUNTING_DEFAULT_PRIOR);
    }

    #[test]
    fn hunting_beliefs_clamp_to_bounds() {
        let mut b = FoxHuntingBeliefs::default_map();
        for _ in 0..100 {
            b.reinforce(Position::new(50, 50), 0.1);
        }
        assert_eq!(b.get(50, 50), HUNTING_MAX_BELIEF);

        for _ in 0..100 {
            b.decay(Position::new(50, 50), 0.1);
        }
        assert_eq!(b.get(50, 50), HUNTING_MIN_BELIEF);
    }

    #[test]
    fn hunting_beliefs_bucketed_by_position() {
        let mut b = FoxHuntingBeliefs::default_map();
        b.reinforce(Position::new(50, 50), 0.3);
        // Same bucket (within 5 tiles).
        assert_eq!(b.get(50, 50), b.get(52, 53));
        // Different bucket.
        assert_ne!(b.get(50, 50), b.get(60, 50));
    }

    #[test]
    fn threat_memory_decays_to_zero() {
        let mut t = FoxThreatMemory::default_map();
        t.record_threat(Position::new(30, 30), 0.8);
        assert!(t.get(30, 30) > 0.0);
        for _ in 0..100 {
            t.decay_all(0.1);
        }
        assert_eq!(t.get(30, 30), 0.0);
    }

    #[test]
    fn threat_memory_accumulates() {
        let mut t = FoxThreatMemory::default_map();
        t.record_threat(Position::new(30, 30), 0.3);
        t.record_threat(Position::new(30, 30), 0.3);
        assert!((t.get(30, 30) - 0.6).abs() < 1e-5);
    }

    #[test]
    fn exploration_visits_saturate() {
        let mut e = FoxExplorationMap::default_map();
        for _ in 0..50 {
            e.visit(Position::new(40, 40));
        }
        assert_eq!(e.get(40, 40), 1.0);
    }

    #[test]
    fn out_of_bounds_returns_default() {
        let b = FoxHuntingBeliefs::default_map();
        assert_eq!(b.get(-1, 50), HUNTING_DEFAULT_PRIOR);
        assert_eq!(b.get(999, 999), HUNTING_DEFAULT_PRIOR);
    }
}
