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
//! - `SpatialConsideration` — computes the cat's Manhattan distance to
//!   a specific landmark (target candidate position or fixed tile) and
//!   passes the distance through a `Curve`. The §L2.10.7 plan-cost
//!   feedback design: landmark distance enters scoring as a curve
//!   contribution, so high-cost candidates are suppressed elastically
//!   without a pathfinder-in-the-loop. Closer-is-better axes use
//!   `Quadratic` / `Power` (sharp falloff); close-enough axes use
//!   `Logistic` (sigmoid plateau); incentive-gradient axes use
//!   `Linear`.
//! - `MarkerConsideration` — reads one ECS marker as a 0/1 gate. Most
//!   marker uses are eligibility filters (§4), but *additive* markers
//!   that contribute to the score (not just gate it) are scored via
//!   this flavor.
//!
//! The concrete flavors here are *pure* — they know how to compute a
//! score given their input. They don't carry the input themselves;
//! that's the evaluator's job (Phase 3a task #5: EvalCtx + evaluate()).

use super::curves::Curve;
use crate::components::physical::Position;
use bevy_ecs::entity::Entity;

// ---------------------------------------------------------------------------
// Scalar consideration
// ---------------------------------------------------------------------------

/// One scalar input, one curve. The canonical §1.2 "concrete numbers /
/// abstract ratings" flavor. Example: `Eat.hunger` = scalar hunger →
/// `Logistic(8, 0.5)` (recalibrated ticket 044).
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
// Spatial consideration — §L2.10.7 plan-cost feedback
// ---------------------------------------------------------------------------

/// One landmark distance, one curve. The §L2.10.7 plan-cost feedback
/// design: the cat's Manhattan distance to a specific landmark
/// (candidate target, fixed tile) enters scoring as a curve
/// contribution. High-cost candidates degrade smoothly via the curve
/// shape without invoking a pathfinder mid-score.
///
/// **Two channels compose** per spec §0.2's elastic-failure rule. The
/// elastic channel here (curve attenuation) and the GOAP hard-fail
/// channel (`replan_count ≥ max_replans` in
/// `crate::components::goap_plan`) are designed to coexist: scoring
/// degrades smoothly as reachability worsens; only when elasticity
/// has run out does `replan_count` fire as the last exit.
///
/// **Curve choice per axis** (§L2.10.7):
/// - `Quadratic` / `Power` — closer-is-better, sharp falloff (hunt,
///   defend-territory, urgent-threat).
/// - `Logistic` — close-enough plateau (routine errands, non-urgent
///   socializing).
/// - `Linear` — incentive gradient (exploration).
///
/// Numeric tuning (curve parameters) is balance-thread work, not
/// substrate scope; this struct commits curve *shape*.
#[derive(Debug, Clone)]
pub struct SpatialConsideration {
    pub name: &'static str,
    /// What the curve measures distance to. Resolved by the evaluator
    /// against `EvalCtx`.
    pub landmark: LandmarkSource,
    /// Tile-range for input normalization. Manhattan distance is
    /// divided by `range` before being passed to the curve, so curve
    /// parameters (especially `Logistic.midpoint`) are written in
    /// `[0, 1]` units regardless of the per-DSE candidate range. A
    /// closer-is-better axis wraps its curve in
    /// `Composite { ..., Invert }`; a farther-is-better axis (Flee,
    /// Avoid) operates directly on the normalized cost.
    pub range: f32,
    pub curve: Curve,
}

/// Where to compute distance from `EvalCtx::self_position`. The
/// evaluator resolves the landmark to a concrete `Position`, takes
/// the Manhattan distance from the cat's position, and runs that
/// distance through the curve.
#[derive(Debug, Clone, Copy)]
pub enum LandmarkSource {
    /// Resolves to `EvalCtx::target_position`. Returns score 0.0 when
    /// the target position is `None` (target-taking DSE without a
    /// candidate). The canonical target-taking-DSE shape: each
    /// candidate's position drives the per-candidate spatial axis.
    TargetPosition,
    /// Fixed tile coordinate. For globally-known landmarks (a unique
    /// hearth, a colony center) or per-cat landmarks resolved at
    /// construction time.
    Tile(Position),
    /// Pinned entity reference. The evaluator resolves the entity's
    /// current `Position` via the `EvalCtx::entity_position` lookup.
    /// Returns score 0.0 when the entity has no resolvable position
    /// (despawned, off-grid).
    Entity(Entity),
    /// Cat-relative anchor — a per-cat-per-tick landmark resolved by
    /// the evaluator via the `EvalCtx::anchor_position` closure. The
    /// canonical self-state-DSE shape: the DSE registers *which kind*
    /// of landmark it wants (e.g. nearest kitchen, own den, prey
    /// belief centroid) and the evaluator resolves the concrete
    /// position from per-cat queries / colony resources / per-tick
    /// snapshots. Returns score 0.0 when the anchor has no resolvable
    /// position (no kitchen built, no threat in range, empty
    /// frontier).
    Anchor(LandmarkAnchor),
}

/// Cat- or fox-relative anchor kind. Identifies *which* landmark the
/// `LandmarkSource::Anchor` variant resolves to; the evaluator's
/// `anchor_position` closure dispatches per-variant against per-tick
/// snapshots, colony resources (e.g. `ColonyLandmarks`), and per-cat
/// component lookups.
///
/// **Design note — unit variants only.** `LandmarkSource` derives
/// `Copy`, so `LandmarkAnchor` must also be `Copy`. Anchor *kind* is
/// the identity; resolution is the closure's job. Adding payload (e.g.
/// `Nearest(TileTag)`) would push the lookup logic up into the DSE
/// definition; keeping it unit-only keeps DSEs declarative and the
/// closure-side dispatch a flat match table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandmarkAnchor {
    // ---- Cat-side colony-resource landmarks (resolved via
    // `ColonyLandmarks` resource — populated at world-gen / on
    // construction events).
    /// Nearest food-stores tile. Eat.
    NearestStores,
    /// Nearest functional kitchen tile. Cook + HerbcraftPrepare.
    NearestKitchen,
    /// Nearest garden tile. Farm.
    NearestGarden,
    /// Nearest forageable-tile cluster (territory boundary). Forage.
    NearestForageableCluster,
    /// Nearest active construction site. Build self-state.
    NearestConstructionSite,
    /// Nearest herb patch (gather source). HerbcraftGather.
    NearestHerbPatch,
    /// Nearest territory perimeter tile (ward placement). HerbcraftWard.
    NearestPerimeterTile,
    /// Coordinator's perch / colony meeting tile. Coordinate.
    CoordinatorPerch,
    /// Anchor for the cat's territory perimeter walk. Patrol.
    TerritoryPerimeterAnchor,

    // ---- Cat-side dynamic landmarks (per-cat queries / per-tick
    // computations).
    /// Cat's preferred sleeping spot — nearest acceptable sleeping
    /// tile. Sleep.
    OwnSleepingSpot,
    /// Nearest threat entity (Manhattan-nearest hostile within
    /// detection range). Flee.
    NearestThreat,
    /// Nearest corrupted tile. Cleanse + DurableWard.
    NearestCorruptedTile,

    // ---- Centroids (per-tick precomputed snapshots).
    /// Centroid of the unexplored-territory frontier. Explore + fox
    /// Dispersing.
    UnexploredFrontierCentroid,
    /// Centroid of high-corruption tiles across the territory.
    /// ColonyCleanse.
    TerritoryCorruptionCentroid,
    /// Centroid of high-prey-belief cells in the focal fox's belief
    /// grid. Fox Hunting.
    PreyBeliefCentroid,
    /// Centroid of nearby cat positions (≤ detection range).
    /// Fox Avoiding.
    CatClusterCentroid,

    // ---- Fox-side specific landmarks.
    /// Focal fox's home den entity position. Fox Resting / Feeding /
    /// DenDefense.
    OwnDen,
    /// Nearest visible (un-guarded) colony store tile from fox POV.
    /// Fox Raiding.
    NearestVisibleStore,
    /// Nearest map-edge tile (escape routing). Fox Fleeing.
    NearestMapEdge,
}

impl SpatialConsideration {
    pub fn new(name: &'static str, landmark: LandmarkSource, range: f32, curve: Curve) -> Self {
        debug_assert!(range > 0.0, "SpatialConsideration::range must be positive");
        Self {
            name,
            landmark,
            range,
            curve,
        }
    }

    /// Evaluate the curve at the normalized cost `distance / range`.
    /// The evaluator resolves landmark → `Position` → Manhattan
    /// distance, then calls this. Curves operate on normalized cost
    /// (`[0, 1+]`) so per-DSE range scales factor out of curve
    /// parameters.
    pub fn score(&self, distance: f32) -> f32 {
        let normalized = distance / self.range;
        self.curve.evaluate(normalized)
    }

    /// Stable label for trace emission. Distinguishes landmark
    /// flavors in §11.3 L2 records without leaking entity ids or
    /// runtime tile coords (which would balloon the trace).
    pub fn landmark_label(&self) -> &'static str {
        match self.landmark {
            LandmarkSource::TargetPosition => "target_position",
            LandmarkSource::Tile(_) => "tile",
            LandmarkSource::Entity(_) => "entity",
            LandmarkSource::Anchor(_) => "anchor",
        }
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
    use crate::ai::curves::{Curve, hangry};

    fn quadratic_unit() -> Curve {
        Curve::Quadratic {
            exponent: 2.0,
            divisor: 1.0,
            shift: 0.0,
        }
    }

    #[test]
    fn scalar_matches_curve_output() {
        let c = ScalarConsideration::new("hunger", hangry());
        // Hangry midpoint after ticket 044 recalibration is 0.5.
        assert!((c.score(0.5) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn spatial_landmark_label_dispatches_per_variant() {
        let target = SpatialConsideration::new(
            "hunt",
            LandmarkSource::TargetPosition,
            10.0,
            quadratic_unit(),
        );
        let tile = SpatialConsideration::new(
            "hearth",
            LandmarkSource::Tile(Position::new(0, 0)),
            10.0,
            quadratic_unit(),
        );
        let entity = SpatialConsideration::new(
            "den",
            LandmarkSource::Entity(Entity::from_raw_u32(1).unwrap()),
            10.0,
            quadratic_unit(),
        );
        let anchor = SpatialConsideration::new(
            "kitchen",
            LandmarkSource::Anchor(LandmarkAnchor::NearestKitchen),
            10.0,
            quadratic_unit(),
        );
        assert_eq!(target.landmark_label(), "target_position");
        assert_eq!(tile.landmark_label(), "tile");
        assert_eq!(entity.landmark_label(), "entity");
        assert_eq!(anchor.landmark_label(), "anchor");
    }

    #[test]
    fn spatial_normalizes_distance_by_range() {
        // Quadratic(exp=2, div=1, shift=0) evaluates `d/R` → `(d/R)^2`.
        let c = SpatialConsideration::new(
            "test",
            LandmarkSource::TargetPosition,
            10.0,
            quadratic_unit(),
        );
        assert!((c.score(0.0) - 0.0).abs() < 1e-4);
        assert!((c.score(5.0) - 0.25).abs() < 1e-4); // half-range → 0.5² = 0.25
        assert!((c.score(10.0) - 1.0).abs() < 1e-4); // full range → 1² = 1
    }

    #[test]
    fn spatial_curve_invert_makes_closer_better() {
        // Composite { Quadratic(2), Invert } evaluates 1 - (d/R)².
        let c = SpatialConsideration::new(
            "closer_is_better",
            LandmarkSource::TargetPosition,
            10.0,
            Curve::Composite {
                inner: Box::new(quadratic_unit()),
                post: super::super::curves::PostOp::Invert,
            },
        );
        assert!((c.score(0.0) - 1.0).abs() < 1e-4); // adjacent → 1
        assert!((c.score(5.0) - 0.75).abs() < 1e-4); // half-range → 1 - 0.25 = 0.75
        assert!((c.score(10.0) - 0.0).abs() < 1e-4); // boundary → 0
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
