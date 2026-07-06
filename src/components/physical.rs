use bevy::math::Vec2;
use bevy_ecs::prelude::*;
use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------
// Fluid movement (0.4.0 "Free Range" — ticket 140 / plan step 6)
// ---------------------------------------------------------------------------

/// Persistent actual velocity, world units (tiles) per tick —
/// **integrator-owned** (`systems::movement::integrate_velocities` is
/// the only production writer). Decision layers never touch this;
/// they express desire via [`DesiredVelocity`] and the integrator
/// turns desire into motion under the acceleration cap. Not
/// serialized into saves: velocity is transient motion state that
/// reconstructs from fresh desires within ~4 ticks of load.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Velocity(pub bevy::math::Vec2);

/// Per-tick movement desire, written by decision layers (migrated
/// resolvers), **consumed-and-cleared by the integrator each tick**.
///
/// `None` is the bisectability invariant of the staged migration: an
/// unmigrated resolver writes `Position` directly and expresses no
/// desire — the integrator then zeroes the mover's [`Velocity`]
/// immediately and moves nothing, so a cat can never be moved twice
/// in one tick (once by a legacy resolver, once by momentum).
/// Momentum exists only across consecutive desire-writing ticks.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DesiredVelocity(pub Option<bevy::math::Vec2>);

/// Terrain-exempt mover (hawks; birds in burst flight — plan steps
/// 9-10). The integrator skips passability/wall-slide for `Flying`
/// entities and applies the map bounds clamp only.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Flying;

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// World-space position. Ticket 491 (Phase 2a of the 135 continuous-position
/// epic) made this a `Vec2<f32>`-backed newtype. For sub-phase 2a, all
/// existing call sites continue to interact with tile-integer coordinates
/// via `Position::new(x: i32, y: i32)` (which snaps to the tile center) and
/// the `x()` / `y()` / `tile()` accessors. Direct Euclidean reads
/// (`pos.world()`) become first-class in sibling sub-phases 2b/2c.
///
/// Wire format: `serde(transparent)` over `Vec2`, so new code paths
/// serialize as a 2-element float array. (A pre-140 revision of this
/// comment referenced a `SavedPosition` shim in `persistence.rs` that
/// never existed — Position serializes transparently and sub-tile
/// positions round-trip as-is.)
///
/// `PartialEq` / `Eq` / `Hash` are keyed on `tile()` — the containing
/// integer grid cell — to preserve the pre-491 `HashMap<Position, _>`
/// invariants without forcing call sites to migrate this sub-phase.
/// Every `Position::new(i32, i32)` snaps to a tile center, so for
/// sub-phase 2a this is equivalent to byte-level Vec2 equality. When
/// sub-phase 2b/2c introduces continuous (sub-tile) positions, the
/// `HashMap<Position, _>` sites will switch to `HashMap<(i32, i32), _>`
/// and the manual `Hash` impl can be revisited.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Position(pub Vec2);

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.tile() == other.tile()
    }
}

impl Eq for Position {}

impl Hash for Position {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tile().hash(state);
    }
}

/// Snapshot of an entity's position at the start of the current tick.
/// Used by the rendering layer to interpolate smooth movement between ticks.
#[derive(Component, Clone, Copy)]
pub struct PreviousPosition(pub Vec2);

impl PreviousPosition {
    pub fn x(&self) -> i32 {
        self.0.x.floor() as i32
    }
    pub fn y(&self) -> i32 {
        self.0.y.floor() as i32
    }
    pub fn set_tile(&mut self, x: i32, y: i32) {
        self.0 = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
    }
}

/// Ticket 129 — Phase 0 of the continuous-position migration epic
/// (#135). World-space smooth position in pixels, computed each render
/// frame from `Position` + `PreviousPosition` + `RenderTickProgress`
/// using a smoothstep ease-in/out curve.
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct RenderPosition(pub Vec2);

impl Position {
    /// Construct from integer tile coordinates. Snaps to the tile center
    /// (`tx + 0.5`, `ty + 0.5`) — preserves containing-tile semantics for
    /// every pre-491 call site (`Position::new(5, 5)` still puts the
    /// entity in tile (5, 5)).
    pub const fn new(x: i32, y: i32) -> Self {
        Position(Vec2::new(x as f32 + 0.5, y as f32 + 0.5))
    }

    /// Construct from a continuous world-space point. Used by future
    /// Euclidean call paths (491b/c); current callers stay on `new`.
    pub const fn from_world(world: Vec2) -> Self {
        Position(world)
    }

    /// Continuous world-space position.
    pub fn world(&self) -> Vec2 {
        self.0
    }

    /// Containing-tile coordinates `(x, y)` — the i32 grid cell this
    /// position falls into. Floor of the inner Vec2.
    pub fn tile(&self) -> (i32, i32) {
        (self.0.x.floor() as i32, self.0.y.floor() as i32)
    }

    /// Containing-tile X coord. Replaces the former `pub x: i32` field
    /// for every read-site in `src/`.
    pub fn x(&self) -> i32 {
        self.0.x.floor() as i32
    }

    /// Containing-tile Y coord. Replaces the former `pub y: i32` field
    /// for every read-site in `src/`.
    pub fn y(&self) -> i32 {
        self.0.y.floor() as i32
    }

    /// In-place tile snap to (x, y). Replaces field-style writes like
    /// `pos.x = nx; pos.y = ny;` in wildlife / prey / snake / hawk
    /// steppers.
    pub fn set_tile(&mut self, x: i32, y: i32) {
        self.0 = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
    }

    /// Default "how close" read — perception, pursuit, social spacing,
    /// any consideration shaped like "how long until I get there."
    /// Returns **world-space Euclidean** distance between the continuous
    /// positions (sub-tile geometry included).
    ///
    /// 140 step 8 — ticket 494 inverted, deliberately. 494 made this
    /// Chebyshev because cats then moved 8-directionally at edge cost 1:
    /// a diagonal-of-1 cost one step, so Chebyshev was the metric that
    /// aligned perception with movement. The Phase II integrator changed
    /// the movement substrate: movers steer along arbitrary headings
    /// under a **Euclidean speed clamp** (`integrate_velocities` —
    /// L∞ would make ground speed direction-dependent, +41% at 45°), so
    /// travel time is now isotropic and the substrate-correct
    /// perception metric is Euclidean again. Diagonal targets read √2
    /// farther than the grid era — that is real travel time now, not a
    /// proprioceptive mismatch.
    ///
    /// Chebyshev is demoted to *tile-tactical* reads — strike range,
    /// adjacency, reach-this-tick — via
    /// [`Position::chebyshev_distance`]. If a comparison encodes "am I
    /// on/next to the tile" (`<= 1`-style), use that; if it encodes
    /// "how far away is it," use this.
    pub fn distance_to(&self, other: &Position) -> f32 {
        self.0.distance(other.0)
    }

    /// Squared "pick nearest" metric via containing tiles. Returns
    /// `dx² + dy²` (Euclidean squared over tile deltas) so it composes
    /// with `min_by_key`, `sort_by_key`, and `Ord::cmp`.
    ///
    /// 140 step 8 — back to Euclidean-squared (the pre-494 body),
    /// matching the now-Euclidean [`Position::distance_to`]. Order-
    /// equivalence caveat: this is *tile-quantized* (i32-composable),
    /// so a nearest-pick can differ from world-space `distance_to`
    /// ordering by sub-tile offsets at near-ties — acceptable for the
    /// "pick nearest" semantic every call site wants. It is NOT order-
    /// equivalent to Chebyshev: former Chebyshev ties (e.g. (3,3) vs
    /// (3,0)) now order by true radial distance — that shift is the
    /// point of the step-8 pivot.
    pub fn tile_distance_squared(&self, other: &Position) -> i32 {
        let (sx, sy) = self.tile();
        let (ox, oy) = other.tile();
        let dx = sx - ox;
        let dy = sy - oy;
        dx * dx + dy * dy
    }

    /// Chebyshev (king-move) distance via containing tiles —
    /// `max(|dx|, |dy|)`. **Tile-tactical reads only** (140 step 8
    /// demotion): strike range, adjacency (`<= 1`), reach-this-tick,
    /// tile-grid A* heuristics. Perception / "how far" reads use the
    /// now-Euclidean [`Position::distance_to`] — under the Phase II
    /// integrator's Euclidean speed clamp, travel time is isotropic
    /// and Chebyshev no longer models it.
    pub fn chebyshev_distance(&self, other: &Position) -> i32 {
        let (sx, sy) = self.tile();
        let (ox, oy) = other.tile();
        (sx - ox).abs().max((sy - oy).abs())
    }

    /// Radial Euclidean distance — world-space, sub-tile geometry
    /// included. Since 140 step 8 this is the same metric as
    /// [`Position::distance_to`]; the name survives as an intent
    /// marker for call sites that measure radial physical space
    /// regardless of the locomotion model — scent diffusion gradients,
    /// ward-glow / wardlight falloff, sound amplitude, line-of-sight
    /// perception falloff. (Pre-step-8 this was tile-quantized —
    /// `floor()`ed coords hid sub-tile geometry; now it reads the
    /// continuous positions directly.)
    pub fn euclidean_distance(&self, other: &Position) -> f32 {
        self.0.distance(other.0)
    }

    /// Manhattan (grid-step) distance via containing tiles. Retained
    /// for test parity and external tooling; sim-code call sites
    /// retired in ticket 492 in favor of `distance_to` (Euclidean) or
    /// `chebyshev_distance` (tactical reach).
    #[deprecated(
        note = "retired by ticket 492; use distance_to / chebyshev_distance (8-direction movement) or euclidean_distance (radial sensing — scent, sound, perception falloff)"
    )]
    pub fn manhattan_distance(&self, other: &Position) -> i32 {
        let (sx, sy) = self.tile();
        let (ox, oy) = other.tile();
        (sx - ox).abs() + (sy - oy).abs()
    }
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// What inflicted an injury. Used by `BodyPartInjury` messages and the
/// `damage_to_body_part` targeting selector. Survives the 095 Phase 1
/// Stage B retirement of legacy `Injury` records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InjurySource {
    /// Regular wildlife combat (hawk, snake, etc.).
    WildlifeCombat,
    /// Shadow fox ambush.
    ShadowFoxAmbush,
    /// Fox confrontation/standoff escalation.
    FoxConfrontation,
    /// Magic misfire (wound transfer).
    MagicMisfire,
    /// Unknown / legacy (pre-tagging injuries).
    Unknown,
}

/// Health component. `current` and `max` are normalised to `[0.0, 1.0]`.
///
/// 095 Phase 1 Stage B retired the `injuries: Vec<Injury>` field — the
/// 13-part `CatBodyModel` is now the canonical anatomical injury
/// substrate. `Health.current` remains the canonical HP scalar; starvation
/// and magic still write it directly.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    /// Ticket 032 — monotonic accumulator of all health drained by the
    /// starvation cascade in `decay_needs`. Used by the death-cause
    /// discriminator under graded-cliff mode (`starvation_cliff_use_legacy
    /// = false`) to attribute deaths to `DeathCause::Starvation` when the
    /// cat may die at `hunger > 0` (the graded-drain regime). Under legacy
    /// mode the field still increments but the discriminator ignores it.
    /// `#[serde(default)]` keeps existing save-files compatible.
    #[serde(default)]
    pub total_starvation_damage: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 1.0,
            max: 1.0,
            total_starvation_damage: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Dead
// ---------------------------------------------------------------------------

/// Cause of death for narrative purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeathCause {
    Starvation,
    OldAge,
    Injury,
}

/// Marker component for dead entities. Dead cats remain in the world for a
/// grace period (narrative, nearby reactions) before despawning.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Dead {
    /// Tick when death occurred.
    pub tick: u64,
    pub cause: DeathCause,
}

// ---------------------------------------------------------------------------
// Smoothstep helper
// ---------------------------------------------------------------------------

/// Standard Hermite smoothstep clamped to [0, 1].
///
/// Returns 0 when `x <= edge0`, 1 when `x >= edge1`, and a smooth curve
/// between.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// Needs
// ---------------------------------------------------------------------------

/// Maslow-hierarchy needs. All values are `f32` in `[0.0, 1.0]` where 1.0
/// means the need is fully satisfied and 0.0 means critically unmet.
///
/// Default values reflect a moderately well-off cat at rest.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Needs {
    // Tier 1 — Physiological
    pub hunger: f32,
    pub energy: f32,
    pub temperature: f32,

    // Tier 2 — Safety
    pub safety: f32,

    // Tier 3 — Belonging
    pub social: f32,
    pub acceptance: f32,
    /// Mating drive. Tier 3 but NOT averaged into belonging_satisfaction — only
    /// used as a scoring input for the Mate action.
    #[serde(default = "default_mating")]
    pub mating: f32,

    // Tier 4 — Esteem
    pub respect: f32,
    pub mastery: f32,

    // Tier 5 — Self-actualisation
    pub purpose: f32,
}

fn default_mating() -> f32 {
    1.0
}

impl Default for Needs {
    fn default() -> Self {
        Self {
            hunger: 1.0,
            energy: 0.8,
            temperature: 0.9,
            safety: 1.0,
            social: 0.6,
            acceptance: 0.5,
            mating: 1.0,
            respect: 0.5,
            mastery: 0.4,
            purpose: 0.2,
        }
    }
}

impl Needs {
    /// Create needs with hunger and energy staggered by position within a
    /// group.  Spreads hunger across `[0.8, 1.0]` and energy across
    /// `[0.65, 0.8]` so that cats don't all hit eat/sleep thresholds at the
    /// same tick — preventing synchronised binge-eating that drains stores in
    /// one wave.  All cats start sated — the stagger just offsets *when* they
    /// first get hungry, not *how* hungry they start.
    pub fn staggered(index: usize, group_size: usize) -> Self {
        let mut needs = Self::default();
        if group_size > 1 {
            let t = index as f32 / (group_size - 1) as f32;
            needs.hunger = 1.0 - t * 0.2; // [0.8, 1.0]
            needs.energy = 0.8 - t * 0.15; // [0.65, 0.8]
        }
        needs.mating = 1.0; // Always starts fully satisfied
        needs
    }

    // -----------------------------------------------------------------------
    // Internal satisfaction helpers
    // -----------------------------------------------------------------------

    /// How satisfied is the physiological level overall?
    ///
    /// Uses the *minimum* of the three needs so that one critical deficiency
    /// suppresses the whole level.
    pub fn physiological_satisfaction(&self) -> f32 {
        let min = self.hunger.min(self.energy).min(self.temperature);
        smoothstep(0.15, 0.65, min)
    }

    fn safety_satisfaction(&self) -> f32 {
        smoothstep(0.2, 0.7, self.safety)
    }

    fn belonging_satisfaction(&self) -> f32 {
        let avg = (self.social + self.acceptance) / 2.0;
        smoothstep(0.15, 0.6, avg)
    }

    fn esteem_satisfaction(&self) -> f32 {
        let avg = (self.respect + self.mastery) / 2.0;
        smoothstep(0.15, 0.6, avg)
    }

    // -----------------------------------------------------------------------
    // Tier suppression
    // -----------------------------------------------------------------------

    /// Returns how freely a given Maslow tier can be pursued, as a value in
    /// `[0.0, 1.0]`.
    ///
    /// Tier 1 is never suppressed (returns 1.0). Each higher tier is the
    /// product of all lower-tier satisfactions so that unmet basics starve
    /// higher motivations.
    ///
    /// | tier  | suppression value |
    /// |-------|-------------------|
    /// | 1     | 1.0 (always)      |
    /// | 2     | physiological satisfaction |
    /// | 3     | phys × safety     |
    /// | 4     | phys × safety × belonging |
    /// | 5     | phys × safety × belonging × esteem |
    pub fn tier_suppression(&self, tier: u8) -> f32 {
        let phys = self.physiological_satisfaction();
        match tier {
            1 => 1.0,
            2 => phys,
            3 => phys * self.safety_satisfaction(),
            4 => phys * self.safety_satisfaction() * self.belonging_satisfaction(),
            5 => {
                phys * self.safety_satisfaction()
                    * self.belonging_satisfaction()
                    * self.esteem_satisfaction()
            }
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Position ---

    #[test]
    fn position_distance() {
        // 140 step 8 — `distance_to` is world-space Euclidean (494
        // inverted): under the Phase II integrator's Euclidean speed
        // clamp, travel time is isotropic, so the (3, 4) offset reads
        // as 5 (√(9+16)) — matching actual travel time again. The
        // 494-era Chebyshev read (4.0) survives only in
        // `chebyshev_distance` for tile-tactical reads.
        let a = Position::new(0, 0);
        let b = Position::new(3, 4);
        let dist = a.distance_to(&b);
        assert!((dist - 5.0).abs() < 1e-5, "expected 5.0, got {dist}");
    }

    #[test]
    fn position_distance_reads_sub_tile_geometry() {
        // World-space means sub-tile offsets are visible: two points
        // inside the same tile are not distance-0 apart.
        let a = Position::from_world(Vec2::new(2.1, 2.1));
        let b = Position::from_world(Vec2::new(2.9, 2.1));
        assert!((a.distance_to(&b) - 0.8).abs() < 1e-5);
        // Tile-tactical reads still see them as co-located.
        assert_eq!(a.chebyshev_distance(&b), 0);
        assert_eq!(a.tile_distance_squared(&b), 0);
    }

    #[test]
    fn position_euclidean_distance_keeps_radial_semantics() {
        // Intent-marker alias for genuinely radial reads (scent
        // diffusion, ward-glow falloff). Same world-space metric as
        // `distance_to` since step 8 (pre-step-8 it was tile-quantized).
        let a = Position::new(0, 0);
        let b = Position::new(3, 4);
        let dist = a.euclidean_distance(&b);
        assert!((dist - 5.0).abs() < 1e-5, "expected 5.0, got {dist}");
    }

    #[test]
    fn tile_distance_squared_is_euclidean_squared_over_tiles() {
        let a = Position::new(0, 0);
        // Step 8: dx² + dy², not Chebyshev². Former Chebyshev ties
        // (3,0) vs (3,3) now order by radial distance.
        assert_eq!(a.tile_distance_squared(&Position::new(3, 0)), 9);
        assert_eq!(a.tile_distance_squared(&Position::new(3, 3)), 18);
        assert_eq!(a.tile_distance_squared(&Position::new(3, 4)), 25);
    }

    #[test]
    #[allow(deprecated)]
    fn position_manhattan() {
        let a = Position::new(1, 2);
        let b = Position::new(4, 6);
        assert_eq!(a.manhattan_distance(&b), 7);
    }

    #[test]
    fn position_chebyshev_cardinal_and_diagonal() {
        let origin = Position::new(0, 0);
        // Cardinal: Chebyshev == Manhattan.
        assert_eq!(origin.chebyshev_distance(&Position::new(3, 0)), 3);
        assert_eq!(origin.chebyshev_distance(&Position::new(0, 4)), 4);
        // Diagonal: Chebyshev picks the larger leg, not their sum.
        assert_eq!(origin.chebyshev_distance(&Position::new(3, 4)), 4);
        assert_eq!(origin.chebyshev_distance(&Position::new(-2, 5)), 5);
        // Self: zero.
        assert_eq!(origin.chebyshev_distance(&origin), 0);
    }

    #[test]
    fn position_distance_to_self_is_zero() {
        let p = Position::new(5, -3);
        assert_eq!(p.distance_to(&p), 0.0);
    }

    // --- Smoothstep ---

    #[test]
    fn smoothstep_at_boundaries() {
        assert_eq!(smoothstep(0.2, 0.7, 0.2), 0.0);
        assert_eq!(smoothstep(0.2, 0.7, 0.7), 1.0);
    }

    #[test]
    fn smoothstep_below_edge0_clamps_to_zero() {
        assert_eq!(smoothstep(0.2, 0.7, 0.0), 0.0);
    }

    #[test]
    fn smoothstep_above_edge1_clamps_to_one() {
        assert_eq!(smoothstep(0.2, 0.7, 1.0), 1.0);
    }

    #[test]
    fn smoothstep_midpoint_is_half() {
        let mid = smoothstep(0.0, 1.0, 0.5);
        assert!((mid - 0.5).abs() < 1e-5, "expected 0.5, got {mid}");
    }

    // --- Default needs ---

    #[test]
    fn default_needs_values() {
        let n = Needs::default();
        assert_eq!(n.hunger, 1.0);
        assert_eq!(n.energy, 0.8);
        assert_eq!(n.temperature, 0.9);
        assert_eq!(n.safety, 1.0);
        assert_eq!(n.social, 0.6);
        assert_eq!(n.acceptance, 0.5);
        assert_eq!(n.respect, 0.5);
        assert_eq!(n.mastery, 0.4);
        assert_eq!(n.purpose, 0.2);
    }

    // --- Suppression: starving cat ---

    #[test]
    fn suppression_starving_cat() {
        let mut n = Needs::default();
        // Drive hunger critical
        n.hunger = 0.05;
        n.energy = 0.05;

        let t1 = n.tier_suppression(1);
        let t2 = n.tier_suppression(2);
        let t3 = n.tier_suppression(3);
        let t4 = n.tier_suppression(4);
        let t5 = n.tier_suppression(5);

        assert_eq!(t1, 1.0, "tier 1 should always be 1.0");
        // Physiological satisfaction near-zero → tiers 2+ heavily suppressed
        assert!(t2 < 0.1, "tier 2 should be heavily suppressed, got {t2}");
        assert!(t3 < 0.1, "tier 3 should be heavily suppressed, got {t3}");
        assert!(t4 < 0.1, "tier 4 should be heavily suppressed, got {t4}");
        assert!(t5 < 0.1, "tier 5 should be heavily suppressed, got {t5}");

        // Each higher tier ≤ the one below (monotone)
        assert!(t2 >= t3);
        assert!(t3 >= t4);
        assert!(t4 >= t5);
    }

    // --- Suppression: well-fed cat ---

    #[test]
    fn suppression_well_fed_cat() {
        let mut n = Needs::default();
        // All needs comfortably met
        n.hunger = 0.9;
        n.energy = 0.9;
        n.temperature = 0.9;
        n.safety = 0.9;
        n.social = 0.9;
        n.acceptance = 0.9;
        n.respect = 0.9;
        n.mastery = 0.9;

        let t5 = n.tier_suppression(5);
        assert!(
            t5 > 0.7,
            "well-fed cat's tier 5 should be mostly unsuppressed, got {t5}"
        );
    }
}
