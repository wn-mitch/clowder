use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

/// Elapsed-tick width of one dispersion window. Matches the bucket the
/// ticket-490 A/B diagnosis used (spawn clump resolves by ~1500; the
/// +3000..6000 / +6000..12000 windows are where the healthy ~24-tile
/// spread vs the ~4.7-tile cuddle puddle separates).
pub const DISPERSION_WINDOW_TICKS: u64 = 3_000;

/// Founder spatial-dispersion accumulator (ticket 490 canary).
///
/// The cuddle-puddle defect was invisible to every event-count gate —
/// courtship/grooming tallies stayed flat while founders collapsed from
/// ~24 to ~4.7 tiles mean distance-to-centroid. This resource accumulates
/// that spatial statistic per elapsed-tick window so the headless footer
/// (and `just verdict`'s absolute-floor check) can see it.
///
/// Sampled inside `emit_cat_snapshots` (no new system — instrumentation
/// writes only, never read by sim behavior, so it cannot perturb seed
/// determinism).
#[derive(Resource, Debug, Default)]
pub struct FounderDispersionStats {
    /// Absolute tick the run started at (ticks on disk are absolute,
    /// ≈ 1.2 M). Seeded by `build_new_world` beside `ColonyScore`.
    pub run_start_tick: u64,
    /// window index (elapsed / `DISPERSION_WINDOW_TICKS`) →
    /// (sum of per-sample mean distance-to-centroid, sample count).
    /// BTreeMap so footer serialization order is stable.
    pub windows: BTreeMap<u64, (f64, u64)>,
}

impl FounderDispersionStats {
    /// Record one sample: the mean Euclidean distance-to-centroid of the
    /// living founder set at `tick` (absolute).
    pub fn record(&mut self, tick: u64, mean_dist: f64) {
        let elapsed = tick.saturating_sub(self.run_start_tick);
        let window = elapsed / DISPERSION_WINDOW_TICKS;
        let entry = self.windows.entry(window).or_insert((0.0, 0));
        entry.0 += mean_dist;
        entry.1 += 1;
    }
}

/// Mean Euclidean distance-to-centroid for a set of positions (tiles).
/// Returns `None` for fewer than 2 positions (dispersion of a singleton
/// is trivially 0 and would dilute the window average).
pub fn mean_dist_to_centroid(positions: &[(f32, f32)]) -> Option<f64> {
    if positions.len() < 2 {
        return None;
    }
    let n = positions.len() as f64;
    let (sx, sy) = positions.iter().fold((0.0f64, 0.0f64), |(sx, sy), (x, y)| {
        (sx + *x as f64, sy + *y as f64)
    });
    let (cx, cy) = (sx / n, sy / n);
    let sum_dist: f64 = positions
        .iter()
        .map(|(x, y)| {
            let dx = *x as f64 - cx;
            let dy = *y as f64 - cy;
            (dx * dx + dy * dy).sqrt()
        })
        .sum();
    Some(sum_dist / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton_and_empty_yield_none() {
        assert!(mean_dist_to_centroid(&[]).is_none());
        assert!(mean_dist_to_centroid(&[(3.0, 4.0)]).is_none());
    }

    #[test]
    fn symmetric_pair_distance() {
        // Centroid (5, 0); each point 5 tiles away.
        let d = mean_dist_to_centroid(&[(0.0, 0.0), (10.0, 0.0)]).unwrap();
        assert!((d - 5.0).abs() < 1e-9);
    }

    #[test]
    fn coincident_points_zero_dispersion() {
        let d = mean_dist_to_centroid(&[(7.0, 7.0), (7.0, 7.0), (7.0, 7.0)]).unwrap();
        assert!(d.abs() < 1e-9);
    }

    #[test]
    fn record_buckets_by_elapsed_window() {
        let mut stats = FounderDispersionStats {
            run_start_tick: 1_200_000,
            ..Default::default()
        };
        stats.record(1_200_100, 10.0); // elapsed 100 → window 0
        stats.record(1_203_500, 20.0); // elapsed 3500 → window 1
        stats.record(1_204_000, 30.0); // elapsed 4000 → window 1
        assert_eq!(stats.windows.get(&0), Some(&(10.0, 1)));
        let (sum, n) = stats.windows.get(&1).copied().unwrap();
        assert!((sum - 50.0).abs() < 1e-9);
        assert_eq!(n, 2);
    }
}
