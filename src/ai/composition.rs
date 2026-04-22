//! Multi-consideration composition — §3 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! A DSE reduces N consideration scores (each in `[0, 1]`) to one DSE
//! score via one of three composition modes:
//!
//! - `CompensatedProduct` — every axis is a gate; zero-on-any zeroes
//!   the DSE. Compensation factor per §3.2 softens the N-axis bias.
//! - `WeightedSum` — axes are trade-off drivers; weights sum to 1.0.
//! - `Max` — retiring under §L2.10 sibling-DSE split; implemented
//!   because the spec committed to all three modes, but no in-tree
//!   DSE registers with `Max` post-3c.
//!
//! `§3.3.1` fixes weight-expression mode per composition: `RtM` for CP
//! (weights in `[0, 1]` as per-axis max-contribution coefficients) and
//! `RtEO` for WS (weights sum to 1.0). These invariants are enforced
//! at construction via `Composition::compensated_product` /
//! `Composition::weighted_sum` factory helpers.

// ---------------------------------------------------------------------------
// CompositionMode + Composition
// ---------------------------------------------------------------------------

/// Per-DSE composition selection. Matches §3.1 exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionMode {
    CompensatedProduct,
    WeightedSum,
    Max,
}

/// Weight-expression mode per §3.3. Chosen by composition:
/// `CompensatedProduct` → RtM, `WeightedSum` → RtEO. `Max` defers to
/// sibling-DSE split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightMode {
    /// Relative-to-max: weights are per-axis max-contribution in `[0, 1]`.
    RelativeToMax,
    /// Relative-to-each-other: weights sum to 1.0.
    RelativeToEachOther,
}

impl CompositionMode {
    pub fn weight_mode(self) -> Option<WeightMode> {
        match self {
            Self::CompensatedProduct => Some(WeightMode::RelativeToMax),
            Self::WeightedSum => Some(WeightMode::RelativeToEachOther),
            Self::Max => None,
        }
    }
}

/// Default compensation strength per §3.2 — `0.75` mirrors big-brain's
/// observable behavior. `0` reproduces pure product; `1` gives
/// geometric mean. Tuning this toward 0 is a regression toward
/// brittle scoring — §3.2's whole argument is why this default exists.
pub const DEFAULT_COMPENSATION_STRENGTH: f32 = 0.75;

/// Composition shape attached to each DSE, carrying enough metadata to
/// reduce N considerations to one score. Prefer the `compensated_product`
/// / `weighted_sum` / `max` factory methods — they validate weight
/// invariants per §3.3.1.
#[derive(Debug, Clone)]
pub struct Composition {
    pub mode: CompositionMode,
    /// Per-axis weights. Under `CompensatedProduct` each weight lives
    /// in `[0, 1]` and scales the axis's ceiling. Under `WeightedSum`
    /// weights sum to 1.0 (enforced to tolerance at construction).
    /// `Max` ignores weights; the field stays empty.
    pub weights: Vec<f32>,
    /// §3.2 compensation strength in `[0, 1]`. Ignored outside CP.
    pub compensation_strength: f32,
}

impl Composition {
    /// Build a `CompensatedProduct` composition. `weights` must all be
    /// in `[0, 1]` per §3.3.1's RtM rule. Panics on violation (plugin-
    /// load-time error, not per-tick).
    pub fn compensated_product(weights: Vec<f32>) -> Self {
        assert!(
            weights.iter().all(|w| (0.0..=1.0).contains(w)),
            "CompensatedProduct weights must be in [0, 1] (RtM); got {weights:?}"
        );
        Self {
            mode: CompositionMode::CompensatedProduct,
            weights,
            compensation_strength: DEFAULT_COMPENSATION_STRENGTH,
        }
    }

    /// Build a `WeightedSum` composition. Weights must sum to 1.0
    /// within `1e-4` tolerance per §3.3.1's RtEO rule.
    pub fn weighted_sum(weights: Vec<f32>) -> Self {
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-4,
            "WeightedSum weights must sum to 1.0 (RtEO); got {weights:?} (sum {sum})"
        );
        Self {
            mode: CompositionMode::WeightedSum,
            weights,
            compensation_strength: 0.0,
        }
    }

    /// Build a `Max` composition. Retiring per §L2.10; retained for
    /// spec coverage.
    pub fn max(axis_count: usize) -> Self {
        Self {
            mode: CompositionMode::Max,
            weights: vec![1.0; axis_count],
            compensation_strength: 0.0,
        }
    }

    /// Override the compensation strength. Caller-facing for tests and
    /// for the (rare) DSE that wants to tune its CP softness. In-spec
    /// production use should rely on `DEFAULT_COMPENSATION_STRENGTH`.
    pub fn with_compensation(mut self, strength: f32) -> Self {
        self.compensation_strength = strength.clamp(0.0, 1.0);
        self
    }

    /// Reduce N consideration scores to one DSE score.
    ///
    /// Preconditions: `considerations.len() == self.weights.len()` for
    /// CP and WS. The caller (the DSE evaluator) guarantees this.
    pub fn compose(&self, considerations: &[f32]) -> f32 {
        if considerations.is_empty() {
            return 0.0;
        }
        match self.mode {
            CompositionMode::CompensatedProduct => {
                compensated_product(considerations, &self.weights, self.compensation_strength)
            }
            CompositionMode::WeightedSum => weighted_sum(considerations, &self.weights),
            CompositionMode::Max => considerations
                .iter()
                .copied()
                .fold(0.0_f32, f32::max),
        }
    }
}

// ---------------------------------------------------------------------------
// Reducers
// ---------------------------------------------------------------------------

/// Compensated-product reduction per §3.2:
///
/// ```text
/// raw         = Π wᵢ · cᵢ
/// compensated = raw ^ (1 / n)
/// final       = lerp(raw, compensated, strength)
/// ```
///
/// The geometric-mean branch is Mark's compensation for N-axis bias —
/// a 6-axis product at `c=0.7` per axis is `0.117` raw vs. `0.7` after
/// geometric-mean compensation. `strength = 0.75` (§3.2 default)
/// pulls most of the way toward the geometric mean while keeping the
/// soft-gate semantic (any axis ≈ 0 still zeroes the output).
fn compensated_product(considerations: &[f32], weights: &[f32], strength: f32) -> f32 {
    debug_assert_eq!(considerations.len(), weights.len());
    let n = considerations.len();
    if n == 0 {
        return 0.0;
    }
    let raw: f32 = considerations
        .iter()
        .zip(weights.iter())
        .map(|(c, w)| (c * w).clamp(0.0, 1.0))
        .product();
    // n == 1 short-circuit: compensation is a no-op; return raw.
    if n == 1 {
        return raw;
    }
    let compensated = raw.powf(1.0 / n as f32);
    let final_score = raw + strength * (compensated - raw);
    final_score.clamp(0.0, 1.0)
}

/// Weighted-sum reduction: `Σ wᵢ · cᵢ`. Weights sum to 1.0 per RtEO,
/// so the normalization in ch 13 §"Weighted Sums" (divide by Σw) is
/// baked into the invariant rather than computed per call.
fn weighted_sum(considerations: &[f32], weights: &[f32]) -> f32 {
    debug_assert_eq!(considerations.len(), weights.len());
    let sum: f32 = considerations
        .iter()
        .zip(weights.iter())
        .map(|(c, w)| c * w)
        .sum();
    sum.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    // --- CompensatedProduct ---

    #[test]
    fn cp_zero_axis_zeroes_score() {
        let c = Composition::compensated_product(vec![1.0, 1.0]);
        assert_eq!(c.compose(&[0.0, 0.9]), 0.0);
    }

    #[test]
    fn cp_single_axis_matches_weight_times_input() {
        let c = Composition::compensated_product(vec![0.8]);
        assert!(approx(c.compose(&[0.5]), 0.5 * 0.8));
    }

    #[test]
    fn cp_default_compensation_softens_n_axis_bias() {
        let c = Composition::compensated_product(vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        // All axes at 0.7: raw product = 0.7^6 ≈ 0.117. Compensation
        // lerps 75% toward geometric mean (≈ 0.7). Should land well
        // above the raw product.
        let score = c.compose(&[0.7; 6]);
        assert!(
            score > 0.5,
            "expected compensation to soften the 6-axis product; got {score}"
        );
        assert!(score < 0.75);
    }

    #[test]
    fn cp_compensation_zero_reproduces_raw_product() {
        let c = Composition::compensated_product(vec![1.0, 1.0, 1.0]).with_compensation(0.0);
        let score = c.compose(&[0.8, 0.8, 0.8]);
        assert!(approx(score, 0.8 * 0.8 * 0.8));
    }

    #[test]
    fn cp_compensation_one_is_geometric_mean() {
        let c = Composition::compensated_product(vec![1.0, 1.0, 1.0]).with_compensation(1.0);
        let score = c.compose(&[0.8, 0.8, 0.8]);
        // Geometric mean of three 0.8s is 0.8.
        assert!(approx(score, 0.8));
    }

    #[test]
    fn cp_respects_weights() {
        let c = Composition::compensated_product(vec![1.0, 0.5]).with_compensation(0.0);
        // Raw: (1.0 · 0.8) × (0.5 · 0.6) = 0.24.
        assert!(approx(c.compose(&[0.8, 0.6]), 0.24));
    }

    #[test]
    #[should_panic(expected = "RtM")]
    fn cp_rejects_out_of_range_weights() {
        Composition::compensated_product(vec![1.5]);
    }

    // --- WeightedSum ---

    #[test]
    fn ws_weighted_mean() {
        let c = Composition::weighted_sum(vec![0.5, 0.5]);
        assert!(approx(c.compose(&[0.8, 0.2]), 0.5));
    }

    #[test]
    fn ws_single_axis_can_drive_action() {
        let c = Composition::weighted_sum(vec![0.7, 0.2, 0.1]);
        // Other axes zero; driving axis alone contributes 0.7.
        assert!(approx(c.compose(&[1.0, 0.0, 0.0]), 0.7));
    }

    #[test]
    fn ws_clamps_at_one() {
        let c = Composition::weighted_sum(vec![0.5, 0.5]);
        assert_eq!(c.compose(&[1.0, 1.0]), 1.0);
    }

    #[test]
    #[should_panic(expected = "RtEO")]
    fn ws_rejects_non_unit_weights() {
        Composition::weighted_sum(vec![0.5, 0.3]);
    }

    #[test]
    fn ws_weights_sum_to_one_ok() {
        Composition::weighted_sum(vec![0.1, 0.2, 0.3, 0.4]);
    }

    // --- Max ---

    #[test]
    fn max_picks_largest() {
        let c = Composition::max(3);
        assert!(approx(c.compose(&[0.3, 0.7, 0.5]), 0.7));
    }

    // --- Edge cases ---

    #[test]
    fn empty_considerations_returns_zero() {
        let c = Composition::weighted_sum(vec![1.0]);
        assert_eq!(c.compose(&[]), 0.0);
    }

    #[test]
    fn weight_mode_matches_composition() {
        assert_eq!(
            CompositionMode::CompensatedProduct.weight_mode(),
            Some(WeightMode::RelativeToMax)
        );
        assert_eq!(
            CompositionMode::WeightedSum.weight_mode(),
            Some(WeightMode::RelativeToEachOther)
        );
        assert_eq!(CompositionMode::Max.weight_mode(), None);
    }
}
