//! Consideration trait — §1 of `docs/systems/ai-substrate-refactor.md`.
//!
//! A *consideration* is a named function `input → [0, 1]`. Each DSE
//! composes 4–8 considerations (§1.4) via the `Composition` machinery
//! in [`super::composition`] to produce a single score for its
//! candidate Intention.
//!
//! Three flavors (§1.2), each corresponding to one input shape:
//!
//! - `ScalarConsideration` — reads one `f32` from the cat's state and
//!   passes it through a `Curve`. Covers hunger, boldness, skill,
//!   satisfaction — anything already bounded or easily normalized.
//! - `SpatialConsideration` — samples an `InfluenceMap` at a position
//!   (typically the cat's own position for self-referential axes like
//!   "threat level here", or a candidate target's position for
//!   target-taking DSEs) and passes the sample through a `Curve`.
//! - `MarkerConsideration` — reads one ECS marker as a 0/1 gate. Most
//!   marker uses are eligibility filters (§4), but *additive* markers
//!   that contribute to the score (not just gate it) are scored via
//!   this flavor.
//!
//! The concrete flavors here are *pure* — they know how to compute a
//! score given their input. They don't carry the input themselves;
//! that's the evaluator's job (Phase 3a task #5: EvalCtx + evaluate()).

use super::curves::Curve;

// ---------------------------------------------------------------------------
// Scalar consideration
// ---------------------------------------------------------------------------

/// One scalar input, one curve. The canonical §1.2 "concrete numbers /
/// abstract ratings" flavor. Example: `Eat.hunger` = scalar hunger →
/// `Logistic(8, 0.75)`.
#[derive(Debug, Clone)]
pub struct ScalarConsideration {
    pub name: &'static str,
    pub curve: Curve,
}

impl ScalarConsideration {
    pub fn new(name: &'static str, curve: Curve) -> Self {
        Self { name, curve }
    }

    /// Evaluate the curve at the caller-supplied scalar. Callers are
    /// responsible for fetching the scalar (the evaluator pulls from
    /// `EvalCtx` and dispatches).
    pub fn score(&self, input: f32) -> f32 {
        self.curve.evaluate(input)
    }
}

// ---------------------------------------------------------------------------
// Spatial consideration
// ---------------------------------------------------------------------------

/// One influence-map sample at a position, one curve. §1.2 spatial
/// flavor — the IAM "personal-interest template" (ch 30) as a
/// first-class consideration shape rather than a post-hoc lookup.
///
/// Two `CenterPolicy` values cover the anchor options:
///
/// - `SelfPosition` — sample at the cat's own position. Example:
///   `Flee.threat_nearby` samples fox-scent at `self.pos`.
/// - `TargetPosition` — sample at a candidate target's position.
///   Example: `Hunt.prey_proximity` under §6's target-taking DSE
///   samples the prey map at each candidate's tile.
///
/// The evaluator (Phase 3a task #5) resolves the position given the
/// consideration's `CenterPolicy` and the current `EvalCtx`.
#[derive(Debug, Clone)]
pub struct SpatialConsideration {
    pub name: &'static str,
    /// Reference to a registered channel × faction map via a stable
    /// key. Phase 3a intentionally uses a string key rather than a
    /// typed enum so Phase 2's `InfluenceMap` registry grows
    /// open-set (§5.6.9); mis-keyed lookups surface as a debug-level
    /// warning rather than a compile error.
    pub map_key: MapKey,
    pub center: CenterPolicy,
    pub curve: Curve,
}

/// Where the spatial sample is taken. Resolved by the evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterPolicy {
    SelfPosition,
    /// Sample at the position of the DSE's current candidate target.
    /// Only meaningful for target-taking DSEs (§6).
    TargetPosition,
}

/// Stable key identifying an L1 influence map. Matches
/// `MapMetadata.name` from [`crate::systems::influence_map`].
pub type MapKey = &'static str;

impl SpatialConsideration {
    pub fn new(name: &'static str, map_key: MapKey, center: CenterPolicy, curve: Curve) -> Self {
        Self {
            name,
            map_key,
            center,
            curve,
        }
    }

    /// Evaluate the curve at the caller-supplied map sample.
    pub fn score(&self, sample: f32) -> f32 {
        self.curve.evaluate(sample)
    }
}

// ---------------------------------------------------------------------------
// Marker consideration
// ---------------------------------------------------------------------------

/// One marker query, one weight. §1.2 marker flavor — a boolean
/// presence contributes additively (or gates via composition) rather
/// than via a curve. Most markers are eligibility filters (§4); this
/// flavor is for the rare cases where a marker's *presence* should
/// contribute a scalar bonus (e.g. `Build.site_bonus` as
/// `Piecewise([(0, 0), (1, build_site_bonus)])` today — but see
/// §2.3 which rewrites those as Piecewise curves on a 0/1 marker-
/// presence input).
///
/// Kept as a separate flavor from `ScalarConsideration` because the
/// input source is different: the evaluator reads a `Query<With<M>>`
/// for markers rather than a scalar field. At score time the two
/// flavors are interchangeable, but the crosswalk in §4.4 treats
/// them as distinct categories.
#[derive(Debug, Clone)]
pub struct MarkerConsideration {
    pub name: &'static str,
    /// Marker identifier. Resolved by the evaluator against a
    /// marker-query registry (Phase 3a task #5).
    pub marker: MarkerKey,
    /// Score contribution when the marker is present (absent = 0.0).
    pub present_score: f32,
}

/// Stable key identifying a marker component by type name. Phase 3a
/// uses a `&'static str` to keep the registry open-set — each marker
/// type registers its own lookup closure.
pub type MarkerKey = &'static str;

impl MarkerConsideration {
    pub fn new(name: &'static str, marker: MarkerKey, present_score: f32) -> Self {
        Self {
            name,
            marker,
            present_score: present_score.clamp(0.0, 1.0),
        }
    }

    /// Evaluate to `present_score` if the marker is present, else 0.0.
    pub fn score(&self, present: bool) -> f32 {
        if present {
            self.present_score
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// Consideration enum — evaluator dispatch point
// ---------------------------------------------------------------------------

/// Consideration-shape union. DSEs declare a list of these at
/// construction; the evaluator dispatches per-variant to fetch the
/// appropriate input and score it.
#[derive(Debug, Clone)]
pub enum Consideration {
    Scalar(ScalarConsideration),
    Spatial(SpatialConsideration),
    Marker(MarkerConsideration),
}

impl Consideration {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Scalar(s) => s.name,
            Self::Spatial(s) => s.name,
            Self::Marker(m) => m.name,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::curves::hangry;

    #[test]
    fn scalar_matches_curve_output() {
        let c = ScalarConsideration::new("hunger", hangry());
        assert!((c.score(0.75) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn spatial_center_policy_is_data() {
        let c = SpatialConsideration::new(
            "fox_scent",
            "fox_scent",
            CenterPolicy::SelfPosition,
            hangry(),
        );
        assert_eq!(c.center, CenterPolicy::SelfPosition);
        assert!(c.score(0.75) > 0.0);
    }

    #[test]
    fn marker_present_vs_absent() {
        let m = MarkerConsideration::new("has_site", "HasConstructionSite", 0.3);
        assert_eq!(m.score(true), 0.3);
        assert_eq!(m.score(false), 0.0);
    }

    #[test]
    fn marker_clamps_present_score() {
        let m = MarkerConsideration::new("x", "X", 1.5);
        assert_eq!(m.score(true), 1.0);
    }

    #[test]
    fn consideration_enum_name_dispatch() {
        let s = Consideration::Scalar(ScalarConsideration::new("hunger", hangry()));
        assert_eq!(s.name(), "hunger");
    }
}
