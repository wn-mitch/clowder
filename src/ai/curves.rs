//! Response-curve primitives — §2 of `docs/systems/ai-substrate-refactor.md`.
//!
//! A `Curve` maps a scalar input (typically normalized `[0, 1]`) to a
//! scalar output (also `[0, 1]` — callers that need a wider range
//! should wrap with `Composite`). Each primitive corresponds one-to-one
//! with the §2.1 enum; parameter names match §2.3's assignment table so
//! rows in the spec port verbatim to construction calls here.
//!
//! Implementation follows §2.2: **start function-evaluated.** Every
//! primitive evaluates in <100 ns (even `Logistic`, which is one
//! `exp()`), so the full 920-sample-per-tick budget fits inside
//! <100 µs. LUT backing is a future optimization (§2.2), not a Phase 3
//! deliverable.
//!
//! All primitives clamp their output to `[0, 1]` — callers compose with
//! `Composite { post: PostOp::Clamp { … } }` to shift that envelope.

// ---------------------------------------------------------------------------
// Curve enum
// ---------------------------------------------------------------------------

/// Response curve primitive per §2.1. Evaluated by `Curve::evaluate`.
///
/// **Output envelope:** every variant returns a value in `[0, 1]`. The
/// envelope is a post-condition of each variant's formula; callers who
/// need broader ranges (e.g. a `Linear(intercept=2.0)` lifecycle
/// override per fox `Dispersing`) wrap with `Composite` + a post-op
/// that lifts the ceiling, so the envelope violation is explicit.
#[derive(Debug, Clone)]
pub enum Curve {
    /// `slope · x + intercept`. Default primitive for bounded
    /// personality / skill scalars (§2.3: boldness, diligence, …).
    Linear { slope: f32, intercept: f32 },

    /// `((x − shift) / divisor)^exponent`. IAM distance-falloff template
    /// (ch 14 §"Choose Your Weapon"; IAM ch 30 §30.3). §2.3 anchor:
    /// scarcity curve (`Quadratic(exp=2)`) shared by Hunt / Forage /
    /// Farm / Cook.
    Quadratic {
        exponent: f32,
        divisor: f32,
        shift: f32,
    },

    /// Logistic sigmoid: `1 / (1 + exp(−steepness · (x − midpoint)))`.
    /// §2.3 anchors: hangry (`Logistic(8, 0.75)`), sleep-dep
    /// (`Logistic(10, 0.7)`), loneliness (`Logistic(5, 0.6)`),
    /// flee-or-fight (`Logistic(10, midpoint)`).
    Logistic { steepness: f32, midpoint: f32 },

    /// Inverse-S: `1 − 1 / (1 + exp(−slope · (x − inflection)))`. Used
    /// for satisfaction / calm / alertness-decay (§2.3, §3.5 modifier
    /// triggers).
    Logit { slope: f32, inflection: f32 },

    /// Hand-authored curve — list of `(input, output)` knots, linearly
    /// interpolated between consecutive knots. Knots must be sorted by
    /// input at construction (see `piecewise()`). §2.3 anchors:
    /// diurnal phase, health/safety gating.
    Piecewise { knots: Vec<(f32, f32)> },

    /// `x^exponent / divisor`. Integer-exponent cousin of `Quadratic`
    /// for IAM threat/proximity templates. `exponent` range 1..=4 per
    /// §2.1.
    Polynomial { exponent: u8, divisor: f32 },

    /// Wraps another curve and applies a post-op (clamp, invert, …).
    /// Used for `Composite { inner: Logistic, post: Invert }` (the
    /// §2.3 "inverted-need penalty" anchor) and for `Clamp(min)` floors.
    Composite {
        inner: Box<Curve>,
        post: PostOp,
    },
}

/// Post-composition transform applied by `Curve::Composite`. Matches
/// §2.3's "adjust data" bucket (ch 12 §"Adjusting Data"). `Invert` is
/// `(1 − x)` — the canonical *inverted-need* shape.
#[derive(Debug, Clone, Copy)]
pub enum PostOp {
    /// `1 − x`. Converts a satisfaction scalar to a deficit signal.
    Invert,
    /// Clamp output range. `min <= max` enforced at construction.
    Clamp { min: f32, max: f32 },
    /// Clamp only the lower bound (useful for "keep available" floors
    /// like `Idle.idle_minimum_floor`).
    ClampMin(f32),
    /// Clamp only the upper bound (useful for saturating counts like
    /// `Fight.ally_count`'s `max=cap`).
    ClampMax(f32),
}

impl Curve {
    /// Evaluate the curve at `x`. Output is `[0, 1]` except when
    /// wrapped by a `Composite` whose post-op lifts the ceiling.
    pub fn evaluate(&self, x: f32) -> f32 {
        match self {
            Self::Linear { slope, intercept } => {
                (slope * x + intercept).clamp(0.0, 1.0)
            }
            Self::Quadratic {
                exponent,
                divisor,
                shift,
            } => {
                let base = (x - shift) / divisor;
                base.max(0.0).powf(*exponent).clamp(0.0, 1.0)
            }
            Self::Logistic { steepness, midpoint } => {
                (1.0 / (1.0 + (-steepness * (x - midpoint)).exp())).clamp(0.0, 1.0)
            }
            Self::Logit { slope, inflection } => {
                (1.0 - 1.0 / (1.0 + (-slope * (x - inflection)).exp())).clamp(0.0, 1.0)
            }
            Self::Piecewise { knots } => piecewise_eval(knots, x),
            Self::Polynomial { exponent, divisor } => {
                (x.powi(*exponent as i32) / divisor).clamp(0.0, 1.0)
            }
            Self::Composite { inner, post } => {
                let raw = inner.evaluate(x);
                match *post {
                    PostOp::Invert => (1.0 - raw).clamp(0.0, 1.0),
                    PostOp::Clamp { min, max } => raw.clamp(min, max),
                    PostOp::ClampMin(min) => raw.max(min),
                    PostOp::ClampMax(max) => raw.min(max),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Piecewise helper + constructor
// ---------------------------------------------------------------------------

/// Build a `Piecewise` curve, sorting knots by input so the evaluator
/// can assume monotonicity. Panics if `knots` is empty — an empty
/// piecewise has no meaningful behavior and represents a construction
/// bug (not a runtime state).
pub fn piecewise(mut knots: Vec<(f32, f32)>) -> Curve {
    assert!(!knots.is_empty(), "piecewise curve requires ≥1 knot");
    knots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Curve::Piecewise { knots }
}

fn piecewise_eval(knots: &[(f32, f32)], x: f32) -> f32 {
    // Below the first knot: flat-extend the leftmost value.
    if x <= knots[0].0 {
        return knots[0].1;
    }
    // Above the last knot: flat-extend the rightmost value.
    let last = knots.len() - 1;
    if x >= knots[last].0 {
        return knots[last].1;
    }
    // Find the segment `x` lives in and linearly interpolate.
    for i in 0..last {
        let (x0, y0) = knots[i];
        let (x1, y1) = knots[i + 1];
        if x >= x0 && x <= x1 {
            let t = if (x1 - x0).abs() < f32::EPSILON {
                0.0
            } else {
                (x - x0) / (x1 - x0)
            };
            return y0 + t * (y1 - y0);
        }
    }
    // Unreachable given the bounds checks above, but fall through
    // to the rightmost value rather than panicking.
    knots[last].1
}

// ---------------------------------------------------------------------------
// Named anchors — shared curve shapes from §2.3
// ---------------------------------------------------------------------------

/// Hangry anchor: `Logistic(8, 0.75)` per §2.3. Reused by
/// `Hunt.hunger`, `Forage.hunger`, fox `Hunting.hunger`,
/// fox `Raiding.hunger`.
pub fn hangry() -> Curve {
    Curve::Logistic {
        steepness: 8.0,
        midpoint: 0.75,
    }
}

/// Sleep-dep anchor: `Logistic(10, 0.7)`.
pub fn sleep_dep() -> Curve {
    Curve::Logistic {
        steepness: 10.0,
        midpoint: 0.7,
    }
}

/// Loneliness anchor: `Logistic(5, 0.6)`. Reused by
/// `Groom(other).social`, `Groom(self).affection`.
pub fn loneliness() -> Curve {
    Curve::Logistic {
        steepness: 5.0,
        midpoint: 0.6,
    }
}

/// Scarcity anchor: `Quadratic(exp=2)` with unit divisor/shift — the
/// ch 13 "soldier curve" shape. Reused by Hunt / Forage / Farm / Cook
/// food-scarcity axes.
pub fn scarcity() -> Curve {
    Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    }
}

/// Flee-or-fight anchor: `Logistic(10, midpoint)`. Steepest logistic
/// in the catalog; shared with fox `DenDefense.cub_safety`.
pub fn flee_or_fight(midpoint: f32) -> Curve {
    Curve::Logistic {
        steepness: 10.0,
        midpoint,
    }
}

/// Inverted-need-penalty anchor:
/// `Composite { Logistic(5, 0.3), Invert }`. Reused by Socialize
/// phys-satisfaction and Groom(other) temper penalty.
pub fn inverted_need_penalty() -> Curve {
    Curve::Composite {
        inner: Box::new(Curve::Logistic {
            steepness: 5.0,
            midpoint: 0.3,
        }),
        post: PostOp::Invert,
    }
}

/// Piecewise-threshold anchor: Fight.health / Fight.safety.
pub fn fight_gating() -> Curve {
    piecewise(vec![(0.0, 0.0), (0.3, 0.2), (0.5, 1.0), (1.0, 1.0)])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn linear_identity() {
        let c = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        assert!(approx(c.evaluate(0.0), 0.0, 1e-6));
        assert!(approx(c.evaluate(0.5), 0.5, 1e-6));
        assert!(approx(c.evaluate(1.0), 1.0, 1e-6));
    }

    #[test]
    fn linear_clamps_at_1() {
        let c = Curve::Linear {
            slope: 2.0,
            intercept: 0.0,
        };
        assert_eq!(c.evaluate(1.0), 1.0);
    }

    #[test]
    fn linear_clamps_at_0() {
        let c = Curve::Linear {
            slope: 1.0,
            intercept: -0.5,
        };
        assert_eq!(c.evaluate(0.0), 0.0);
    }

    #[test]
    fn quadratic_exp2() {
        let c = Curve::Quadratic {
            exponent: 2.0,
            divisor: 1.0,
            shift: 0.0,
        };
        assert!(approx(c.evaluate(0.0), 0.0, 1e-6));
        assert!(approx(c.evaluate(0.5), 0.25, 1e-6));
        assert!(approx(c.evaluate(1.0), 1.0, 1e-6));
    }

    #[test]
    fn logistic_midpoint_is_half() {
        let c = hangry();
        assert!(approx(c.evaluate(0.75), 0.5, 1e-4));
    }

    #[test]
    fn logistic_saturates_high_and_low() {
        let c = hangry();
        assert!(c.evaluate(0.0) < 0.01);
        assert!(c.evaluate(1.0) > 0.88);
    }

    #[test]
    fn logistic_steeper_than_linear_near_midpoint() {
        let c = hangry();
        let slope = (c.evaluate(0.8) - c.evaluate(0.7)) / 0.1;
        // Derivative of logistic at midpoint is steepness/4 = 2.0.
        assert!(slope > 1.5, "expected steep slope near midpoint, got {slope}");
    }

    #[test]
    fn logit_is_inverse_s() {
        let c = Curve::Logit {
            slope: 5.0,
            inflection: 0.5,
        };
        // Below inflection → high; above → low.
        assert!(c.evaluate(0.0) > 0.9);
        assert!(c.evaluate(1.0) < 0.1);
        assert!(approx(c.evaluate(0.5), 0.5, 1e-3));
    }

    #[test]
    fn piecewise_linear_between_knots() {
        let c = piecewise(vec![(0.0, 0.0), (0.5, 1.0), (1.0, 1.0)]);
        assert!(approx(c.evaluate(0.0), 0.0, 1e-6));
        assert!(approx(c.evaluate(0.25), 0.5, 1e-6));
        assert!(approx(c.evaluate(0.5), 1.0, 1e-6));
        assert!(approx(c.evaluate(0.75), 1.0, 1e-6));
    }

    #[test]
    fn piecewise_flat_extends_outside_range() {
        let c = piecewise(vec![(0.2, 0.3), (0.8, 0.7)]);
        assert!(approx(c.evaluate(-1.0), 0.3, 1e-6));
        assert!(approx(c.evaluate(5.0), 0.7, 1e-6));
    }

    #[test]
    fn piecewise_sorts_unsorted_knots() {
        let c = piecewise(vec![(1.0, 1.0), (0.0, 0.0), (0.5, 0.5)]);
        assert!(approx(c.evaluate(0.25), 0.25, 1e-6));
    }

    #[test]
    fn fight_gating_matches_anchor_knots() {
        let c = fight_gating();
        assert!(approx(c.evaluate(0.0), 0.0, 1e-6));
        assert!(approx(c.evaluate(0.3), 0.2, 1e-6));
        assert!(approx(c.evaluate(0.5), 1.0, 1e-6));
        assert!(approx(c.evaluate(1.0), 1.0, 1e-6));
    }

    #[test]
    fn polynomial_cubic() {
        let c = Curve::Polynomial {
            exponent: 3,
            divisor: 1.0,
        };
        assert!(approx(c.evaluate(0.5), 0.125, 1e-6));
        assert!(approx(c.evaluate(1.0), 1.0, 1e-6));
    }

    #[test]
    fn composite_invert() {
        let c = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            }),
            post: PostOp::Invert,
        };
        assert!(approx(c.evaluate(0.25), 0.75, 1e-6));
    }

    #[test]
    fn composite_clamp_floor() {
        let c = Curve::Composite {
            inner: Box::new(Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            }),
            post: PostOp::ClampMin(0.2),
        };
        assert!(approx(c.evaluate(0.0), 0.2, 1e-6));
        assert!(approx(c.evaluate(0.5), 0.5, 1e-6));
    }

    #[test]
    fn inverted_need_penalty_shape() {
        let c = inverted_need_penalty();
        // At high phys_satisfaction (x=1), penalty is low.
        assert!(c.evaluate(1.0) < 0.1);
        // At low phys_satisfaction (x=0), penalty is high.
        assert!(c.evaluate(0.0) > 0.7);
    }

    #[test]
    fn scarcity_anchor_shape() {
        let c = scarcity();
        // Soldier curve: low at abundance, sharp rise toward full scarcity.
        assert!(c.evaluate(0.25) < 0.1);
        assert!(c.evaluate(0.75) > 0.5);
    }

    #[test]
    fn loneliness_anchor_midpoint() {
        let c = loneliness();
        assert!(approx(c.evaluate(0.6), 0.5, 1e-4));
    }

    #[test]
    fn flee_or_fight_anchor_steep() {
        let c = flee_or_fight(0.5);
        // Steepness 10: very sharp transition near midpoint.
        assert!(c.evaluate(0.4) < 0.3);
        assert!(c.evaluate(0.6) > 0.7);
    }

    #[test]
    #[should_panic]
    fn piecewise_rejects_empty_knots() {
        let _ = piecewise(vec![]);
    }
}
