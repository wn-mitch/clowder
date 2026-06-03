use bevy_ecs::prelude::*;

/// 293: uninformative starting belief for unobserved colony buckets.
/// Matches the legacy `HuntingPriors::DEFAULT_PRIOR` value (0.5) — keeps
/// the `best_prey_direction` "above neutral" semantic continuous across
/// the proxy retirement.
pub const DEFAULT_PRIOR: f32 = 0.5;
/// 293: HuntingPriors-style 5-tile bucket size, retained as a Resource
/// invariant. Matches
/// [`crate::systems::belief_aggregation::LOCATION_BUCKET_SIZE`] so the
/// per-cat `LocationBeliefs` keys map 1:1 into colony cells.
const BUCKET_SIZE: i32 = 5;

/// Colony-wide hunting belief grid — read by the `HuntingBeliefSnapshot`
/// downsampler in `snapshot.rs`. Values are **derived** each cadence
/// from per-cat `LocationBeliefs.prey_yield` via
/// [`crate::systems::belief_aggregation::aggregate_location_belief_snapshot`];
/// the Resource itself is a passive output buffer kept in this shape so
/// the visualization payload is unchanged from pre-293 runs.
///
/// Ticket 293 retired the underlying per-cat `HuntingPriors` Component
/// and the legacy `absorb` social-transmission pathway. Per-cat reads
/// (`best_prey_direction`) now consult the cat's own `LocationBeliefs`
/// directly; this colony view is for the global-overlay snapshot only.
#[derive(Resource, Debug, Clone)]
pub struct ColonyHuntingMap {
    pub beliefs: Vec<f32>,
    pub grid_w: usize,
    pub grid_h: usize,
}

impl ColonyHuntingMap {
    pub fn new(map_w: i32, map_h: i32) -> Self {
        let bs = BUCKET_SIZE.max(1) as usize;
        let grid_w = (map_w as usize).div_ceil(bs);
        let grid_h = (map_h as usize).div_ceil(bs);
        Self {
            beliefs: vec![DEFAULT_PRIOR; grid_w * grid_h],
            grid_w,
            grid_h,
        }
    }

    /// Reset every cell to [`DEFAULT_PRIOR`]. Called by the rebuild
    /// system before overlaying the substrate-derived values.
    pub fn reset_to_prior(&mut self) {
        for cell in self.beliefs.iter_mut() {
            *cell = DEFAULT_PRIOR;
        }
    }

    /// Belief at the given world tile, or `DEFAULT_PRIOR` if the tile is
    /// out of grid bounds. Convenience for consumers that don't want to
    /// compute bucket indices themselves.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 {
            return DEFAULT_PRIOR;
        }
        let bx = (x / BUCKET_SIZE) as usize;
        let by = (y / BUCKET_SIZE) as usize;
        if bx >= self.grid_w || by >= self.grid_h {
            return DEFAULT_PRIOR;
        }
        self.beliefs[by * self.grid_w + bx]
    }
}

impl Default for ColonyHuntingMap {
    fn default() -> Self {
        Self::new(120, 90)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_to_prior_clears_all_cells() {
        let mut colony = ColonyHuntingMap::default();
        colony.beliefs[0] = 0.9;
        colony.reset_to_prior();
        assert_eq!(colony.beliefs[0], DEFAULT_PRIOR);
    }

    #[test]
    fn get_returns_default_for_out_of_bounds() {
        let colony = ColonyHuntingMap::default();
        assert_eq!(colony.get(-1, -1), DEFAULT_PRIOR);
        assert_eq!(colony.get(99999, 99999), DEFAULT_PRIOR);
    }
}
