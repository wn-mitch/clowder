//! Per-tick corruption-landmark cache — §L2.10.7 anchor resolution.
//!
//! Holds the intensity-weighted centroid of corrupted tiles across the
//! colony map. Recomputed each tick by `update_corruption_centroid` in
//! `systems/corruption.rs` (or wherever the corruption decay/spread
//! system runs). Read by:
//!
//! - `ColonyCleanseDse` (B12) through
//!   [`LandmarkAnchor::TerritoryCorruptionCentroid`] —
//!   territory-wide cleansing urgency tracks the corrupted region's
//!   geographic centroid.
//!
//! Caching avoids rescanning a 120×90 grid each scoring tick across
//! 50 cats. Recompute cost is dominated by the map iteration
//! (~100 µs); per-tick cost is well within the AI budget.
//!
//! **Why intensity-weighted, not threshold-based.** The centroid of
//! "tiles with corruption ≥ threshold" is brittle near the threshold
//! boundary — a small corruption flux flips many tiles in/out of the
//! set, making the centroid jitter. Intensity-weighting (each tile
//! contributes `corruption × position`) treats the field smoothly so
//! the centroid moves continuously with the corruption pattern.

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// Per-tick cached centroid of the colony's corruption field. `None`
/// when no tile has corruption above the floor (clean colony).
#[derive(Resource, Default, Debug, Clone)]
pub struct CorruptionLandmarks {
    /// Intensity-weighted centroid: `Σ(c × pos) / Σ(c)` over all
    /// tiles where `corruption > floor`. Read by ColonyCleanse via
    /// [`LandmarkAnchor::TerritoryCorruptionCentroid`].
    centroid: Option<Position>,
}

/// Floor below which a tile's corruption contribution is ignored. Set
/// just above the noise level so the centroid tracks meaningful
/// corruption patterns rather than ambient drift.
pub const CORRUPTION_FLOOR: f32 = 0.05;

impl CorruptionLandmarks {
    /// Recompute the cached intensity-weighted centroid from the
    /// supplied tile-corruption sampler. `width × height` is the map
    /// extent; `corruption_at(x, y)` returns the corruption value at
    /// that coord (must be in `[0, 1]`).
    ///
    /// Decoupled from `Map` directly so the maintenance system can
    /// pass a borrow without circular dependencies through the
    /// resources module.
    pub fn recompute<F>(&mut self, width: i32, height: i32, mut corruption_at: F)
    where
        F: FnMut(i32, i32) -> f32,
    {
        let mut weighted_x: f64 = 0.0;
        let mut weighted_y: f64 = 0.0;
        let mut total: f64 = 0.0;
        for y in 0..height {
            for x in 0..width {
                let c = corruption_at(x, y);
                if c > CORRUPTION_FLOOR {
                    let cw = c as f64;
                    weighted_x += cw * x as f64;
                    weighted_y += cw * y as f64;
                    total += cw;
                }
            }
        }
        self.centroid = (total > 0.0).then(|| {
            Position::new(
                (weighted_x / total).round() as i32,
                (weighted_y / total).round() as i32,
            )
        });
    }

    /// Cached intensity-weighted centroid. `None` until the first
    /// recompute or when no corruption is above [`CORRUPTION_FLOOR`].
    pub fn centroid(&self) -> Option<Position> {
        self.centroid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centroid_starts_none() {
        let lm = CorruptionLandmarks::default();
        assert!(lm.centroid().is_none());
    }

    #[test]
    fn centroid_is_none_for_clean_colony() {
        let mut lm = CorruptionLandmarks::default();
        lm.recompute(10, 10, |_, _| 0.0);
        assert!(lm.centroid().is_none());
    }

    #[test]
    fn centroid_tracks_intensity_weighted_mean() {
        // 10×10 grid. One hot tile at (8, 8) with corruption=1.0,
        // one mild tile at (2, 2) with corruption=0.2. Weighted
        // centroid: (1.0*8 + 0.2*2) / 1.2 = 8.4/1.2 = 7.0;
        // (1.0*8 + 0.2*2) / 1.2 = 7.0.
        let mut lm = CorruptionLandmarks::default();
        lm.recompute(10, 10, |x, y| match (x, y) {
            (8, 8) => 1.0,
            (2, 2) => 0.2,
            _ => 0.0,
        });
        let centroid = lm.centroid().expect("two corrupted tiles above floor");
        assert_eq!(centroid, Position::new(7, 7));
    }

    #[test]
    fn centroid_ignores_below_floor_noise() {
        // 5×5 grid. One tile at (4, 4) with high corruption. Other
        // tiles have ambient 0.04 (below floor=0.05) — should be
        // ignored, so centroid lands exactly on the hot tile.
        let mut lm = CorruptionLandmarks::default();
        lm.recompute(5, 5, |x, y| if (x, y) == (4, 4) { 0.9 } else { 0.04 });
        let centroid = lm.centroid().expect("hot tile above floor");
        assert_eq!(centroid, Position::new(4, 4));
    }

    #[test]
    fn centroid_handles_uniform_corruption() {
        // 4×4 grid with uniform corruption of 0.5 — centroid is the
        // geometric center, which floors to (1.5, 1.5) → rounds to
        // (2, 2) (.round() rounds .5 to even or up depending on platform;
        // exact midpoint of 0..3 is 1.5).
        let mut lm = CorruptionLandmarks::default();
        lm.recompute(4, 4, |_, _| 0.5);
        let centroid = lm.centroid().expect("uniform corruption above floor");
        // Σx for 4 cells in each row × 4 rows: per row Σ=0+1+2+3=6,
        // total over 4 rows weighted equally = 6*0.5*4 = 12; total
        // weight = 0.5 * 16 = 8; weighted_x / total = 12/8 = 1.5;
        // .round() in Rust rounds half-away-from-zero → 2.
        assert_eq!(centroid, Position::new(2, 2));
    }
}
