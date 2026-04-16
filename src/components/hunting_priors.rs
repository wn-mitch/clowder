use bevy_ecs::prelude::*;

use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Uninformative starting belief: equal probability of prey presence.
pub const DEFAULT_PRIOR: f32 = 0.5;

/// Belief floor — never fully write off an area.
pub const MIN_BELIEF: f32 = 0.05;

/// Belief ceiling — never treat any area as a guaranteed kill.
pub const MAX_BELIEF: f32 = 0.95;

// ---------------------------------------------------------------------------
// HuntingPriors
// ---------------------------------------------------------------------------

/// Per-cat spatial belief grid over prey abundance.
///
/// The map is divided into square buckets of `bucket_size` tiles. Each bucket
/// stores a single `f32` belief in `[MIN_BELIEF, MAX_BELIEF]`. Beliefs are
/// updated by catches, scent detections, and fruitless searches. There is no
/// time decay — beliefs persist until overwritten by evidence.
#[derive(Component, Debug, Clone)]
pub struct HuntingPriors {
    /// Flat row-major grid of belief values indexed `[by * grid_w + bx]`.
    pub beliefs: Vec<f32>,
    /// Number of buckets along the x axis.
    pub grid_w: usize,
    /// Number of buckets along the y axis.
    pub grid_h: usize,
    /// Side length of each bucket in world tiles.
    pub bucket_size: i32,
}

impl HuntingPriors {
    /// Build a belief grid for a map of `map_w × map_h` tiles with buckets of
    /// `bucket_size` tiles per side.  All beliefs start at `DEFAULT_PRIOR`.
    pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self {
        let bs = bucket_size.max(1) as usize;
        let grid_w = map_w.div_ceil(bs);
        let grid_h = map_h.div_ceil(bs);
        Self {
            beliefs: vec![DEFAULT_PRIOR; grid_w * grid_h],
            grid_w,
            grid_h,
            bucket_size,
        }
    }

    /// Default grid sized for the standard 120×90 map with 5-tile buckets.
    pub fn default_map() -> Self {
        Self::new(120, 90, 5)
    }

    /// Convert a world position to a flat belief index.
    ///
    /// Returns `None` if the position lies outside the grid.
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

    /// Belief at world position `(x, y)`, or `DEFAULT_PRIOR` if out of bounds.
    pub fn get(&self, x: i32, y: i32) -> f32 {
        self.bucket_index(x, y)
            .map(|i| self.beliefs[i])
            .unwrap_or(DEFAULT_PRIOR)
    }

    /// Add `delta` to the belief at `(x, y)` and clamp to `[MIN_BELIEF, MAX_BELIEF]`.
    /// Out-of-bounds positions are silently ignored.
    pub fn update(&mut self, x: i32, y: i32, delta: f32) {
        if let Some(i) = self.bucket_index(x, y) {
            self.beliefs[i] = (self.beliefs[i] + delta).clamp(MIN_BELIEF, MAX_BELIEF);
        }
    }

    /// A successful catch strongly increases belief at `pos`.
    pub fn record_catch(&mut self, pos: &Position) {
        self.update(pos.x, pos.y, 0.15);
    }

    /// Detecting scent weakly increases belief at `pos`.
    pub fn record_scent(&mut self, pos: &Position) {
        self.update(pos.x, pos.y, 0.05);
    }

    /// A fruitless search decreases belief proportional to effort (tiles covered).
    ///
    /// The penalty is `tiles_searched / 2000.0`, so 2000 searched tiles produce
    /// a full −1.0 delta (clamped at `MIN_BELIEF`).
    pub fn record_failed_search(&mut self, pos: &Position, tiles_searched: u64) {
        self.update(pos.x, pos.y, -(tiles_searched as f32 / 2000.0));
    }

    /// Find the highest-belief bucket within `radius` buckets of `pos` that
    /// exceeds `DEFAULT_PRIOR`.
    ///
    /// Returns a unit step `(dx.signum(), dy.signum())` toward the centre of
    /// that bucket, or `None` if no bucket beats the default prior.
    pub fn best_direction(&self, pos: &Position, radius: i32) -> Option<(i32, i32)> {
        let origin_bx = (pos.x / self.bucket_size) as i32;
        let origin_by = (pos.y / self.bucket_size) as i32;

        let mut best_belief = DEFAULT_PRIOR;
        let mut best_bucket: Option<(i32, i32)> = None;

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let bx = origin_bx + dx;
                let by = origin_by + dy;
                if bx < 0 || by < 0 || bx >= self.grid_w as i32 || by >= self.grid_h as i32 {
                    continue;
                }
                let idx = (by as usize) * self.grid_w + (bx as usize);
                let belief = self.beliefs[idx];
                if belief > best_belief {
                    best_belief = belief;
                    best_bucket = Some((bx, by));
                }
            }
        }

        best_bucket.map(|(bx, by)| {
            let center_x = bx * self.bucket_size + self.bucket_size / 2;
            let center_y = by * self.bucket_size + self.bucket_size / 2;
            let dx = center_x - pos.x;
            let dy = center_y - pos.y;
            (dx.signum(), dy.signum())
        })
    }

    /// Blend another cat's beliefs into this one.
    ///
    /// For each bucket where `other` differs from `DEFAULT_PRIOR` by more than
    /// 0.01, the belief is adjusted by `(other - mine) * weight` and clamped.
    /// Buckets at the default prior in `other` are ignored so that uninformed
    /// areas don't regress the learner's hard-won knowledge.
    pub fn learn_from(&mut self, other: &HuntingPriors, weight: f32) {
        let len = self.beliefs.len().min(other.beliefs.len());
        for i in 0..len {
            let theirs = other.beliefs[i];
            if (theirs - DEFAULT_PRIOR).abs() > 0.01 {
                self.beliefs[i] = (self.beliefs[i] + (theirs - self.beliefs[i]) * weight)
                    .clamp(MIN_BELIEF, MAX_BELIEF);
            }
        }
    }
}

impl Default for HuntingPriors {
    fn default() -> Self {
        Self::default_map()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: i32, y: i32) -> Position {
        Position::new(x, y)
    }

    #[test]
    fn new_priors_are_uninformative() {
        let p = HuntingPriors::default_map();
        assert_eq!(p.get(0, 0), DEFAULT_PRIOR);
        assert_eq!(p.get(60, 45), DEFAULT_PRIOR);
        assert_eq!(p.get(119, 89), DEFAULT_PRIOR);
    }

    #[test]
    fn catch_increases_belief() {
        let mut p = HuntingPriors::default_map();
        p.record_catch(&pos(10, 10));

        // The catch site is > default.
        assert!(
            p.get(10, 10) > DEFAULT_PRIOR,
            "catch site should increase belief"
        );

        // A tile in the same 5×5 bucket is also updated.
        assert!(
            p.get(12, 12) > DEFAULT_PRIOR,
            "same-bucket tile should share belief"
        );

        // A distant tile is unaffected.
        assert_eq!(
            p.get(60, 60),
            DEFAULT_PRIOR,
            "distant tile should be unchanged"
        );
    }

    #[test]
    fn failed_search_decreases_belief() {
        let mut p = HuntingPriors::default_map();
        p.record_failed_search(&pos(20, 20), 100);
        assert!(
            p.get(20, 20) < DEFAULT_PRIOR,
            "failed search should reduce belief"
        );
    }

    #[test]
    fn beliefs_clamp_to_bounds() {
        let mut p = HuntingPriors::default_map();
        let catch_pos = pos(5, 5);
        for _ in 0..100 {
            p.record_catch(&catch_pos);
        }
        assert!(
            p.get(5, 5) <= MAX_BELIEF,
            "100 catches must not exceed MAX_BELIEF"
        );

        let mut p2 = HuntingPriors::default_map();
        let fail_pos = pos(5, 5);
        for _ in 0..1000 {
            p2.record_failed_search(&fail_pos, 10);
        }
        assert!(
            p2.get(5, 5) >= MIN_BELIEF,
            "1000 failed searches must not go below MIN_BELIEF"
        );
    }

    #[test]
    fn best_direction_points_toward_high_belief() {
        let mut p = HuntingPriors::default_map();
        // Put high belief to the east of the query origin.
        for _ in 0..3 {
            p.record_catch(&pos(50, 30));
        }
        let dir = p.best_direction(&pos(30, 30), 10);
        assert!(
            dir.is_some(),
            "should find a direction toward high-belief area"
        );
        let (dx, _dy) = dir.unwrap();
        assert_eq!(dx, 1, "best direction should be east (dx=1), got dx={dx}");
    }

    #[test]
    fn best_direction_returns_none_when_flat() {
        let p = HuntingPriors::default_map();
        assert!(
            p.best_direction(&pos(60, 45), 5).is_none(),
            "flat grid should return no direction"
        );
    }

    #[test]
    fn learn_from_blends_beliefs() {
        let mut teacher = HuntingPriors::default_map();
        for _ in 0..3 {
            teacher.record_catch(&pos(10, 10));
        }
        let teacher_belief = teacher.get(10, 10);

        let mut learner = HuntingPriors::default_map();
        learner.learn_from(&teacher, 0.3);

        let learned = learner.get(10, 10);
        assert!(
            learned > DEFAULT_PRIOR,
            "learner should increase belief toward teacher's"
        );
        assert!(
            learned < teacher_belief,
            "learner belief ({learned}) should be less than teacher's ({teacher_belief}) after partial blend"
        );
    }

    #[test]
    fn learn_from_ignores_uninformative_buckets() {
        let mut learner = HuntingPriors::default_map();
        for _ in 0..3 {
            learner.record_catch(&pos(60, 60));
        }
        let before = learner.get(60, 60);

        let teacher = HuntingPriors::default_map(); // entirely flat
        learner.learn_from(&teacher, 0.3);

        let after = learner.get(60, 60);
        assert_eq!(
            before, after,
            "flat teacher should not drag learner's hard-won belief at (60,60)"
        );
    }

    #[test]
    fn negative_evidence_proportional_to_effort() {
        let mut p1 = HuntingPriors::default_map();
        p1.record_failed_search(&pos(10, 10), 100);
        let penalty_100 = DEFAULT_PRIOR - p1.get(10, 10);

        let mut p2 = HuntingPriors::default_map();
        p2.record_failed_search(&pos(10, 10), 20);
        let penalty_20 = DEFAULT_PRIOR - p2.get(10, 10);

        assert!(
            penalty_100 > penalty_20,
            "100-tile search penalty ({penalty_100}) should exceed 20-tile penalty ({penalty_20})"
        );
    }
}
