use bevy::math::Vec2;
use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// World-space grid position.
#[derive(
    Component, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// Snapshot of an entity's grid position at the start of the current tick.
/// Used by the rendering layer to interpolate smooth movement between ticks.
#[derive(Component, Clone, Copy)]
pub struct PreviousPosition {
    pub x: i32,
    pub y: i32,
}

/// Ticket 129 — Phase 0 of the continuous-position migration epic
/// (#127). World-space smooth position in pixels, computed each render
/// frame from `Position` + `PreviousPosition` + `RenderTickProgress`
/// using a smoothstep ease-in/out curve. Sim state (containing tile,
/// pathfinding, perception) still reads `Position` (i32 grid); only
/// the render path consumes this. By Phase 2 (#131), `Position` itself
/// becomes `Vec2<f32>` and this component remains as the per-frame
/// interpolation target without changing its public shape.
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct RenderPosition(pub Vec2);

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another position.
    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dy = (self.y - other.y) as f32;
        (dx * dx + dy * dy).sqrt()
    }

    /// Manhattan (grid-step) distance to another position.
    pub fn manhattan_distance(&self, other: &Position) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// Severity of a wound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InjuryKind {
    Minor,
    Moderate,
    Severe,
}

/// What inflicted an injury.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// A single wound record.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Injury {
    pub kind: InjuryKind,
    pub tick_received: u64,
    pub healed: bool,
    /// What system inflicted this injury.
    pub source: InjurySource,
    /// Where the cat was when the injury was inflicted. Read by
    /// `interoception::own_injury_site` to author the
    /// `LandmarkAnchor::OwnInjurySite` anchor for future TendInjury
    /// DSE consumers. Defaults to map origin via
    /// `default_injury_position` for legacy serde fixtures encoded
    /// before the field existed. Ticket 089.
    #[serde(default = "default_injury_position")]
    pub at: Position,
}

fn default_injury_position() -> Position {
    Position::new(0, 0)
}

/// Health component. `current` and `max` are normalised to `[0.0, 1.0]`.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    pub injuries: Vec<Injury>,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 1.0,
            max: 1.0,
            injuries: Vec::new(),
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
    // Level 1 — Physiological
    pub hunger: f32,
    pub energy: f32,
    pub temperature: f32,

    // Level 2 — Safety
    pub safety: f32,

    // Level 3 — Belonging
    pub social: f32,
    pub acceptance: f32,
    /// Mating drive. L3 but NOT averaged into belonging_satisfaction — only
    /// used as a scoring input for the Mate action.
    #[serde(default = "default_mating")]
    pub mating: f32,

    // Level 4 — Esteem
    pub respect: f32,
    pub mastery: f32,

    // Level 5 — Self-actualisation
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
    // Level suppression
    // -----------------------------------------------------------------------

    /// Returns how freely a given Maslow level can be pursued, as a value in
    /// `[0.0, 1.0]`.
    ///
    /// Level 1 is never suppressed (returns 1.0). Each higher level is the
    /// product of all lower-level satisfactions so that unmet basics starve
    /// higher motivations.
    ///
    /// | level | suppression value |
    /// |-------|-------------------|
    /// | 1     | 1.0 (always)      |
    /// | 2     | physiological satisfaction |
    /// | 3     | phys × safety     |
    /// | 4     | phys × safety × belonging |
    /// | 5     | phys × safety × belonging × esteem |
    pub fn level_suppression(&self, level: u8) -> f32 {
        let phys = self.physiological_satisfaction();
        match level {
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
        let a = Position::new(0, 0);
        let b = Position::new(3, 4);
        let dist = a.distance_to(&b);
        assert!((dist - 5.0).abs() < 1e-5, "expected 5.0, got {dist}");
    }

    #[test]
    fn position_manhattan() {
        let a = Position::new(1, 2);
        let b = Position::new(4, 6);
        assert_eq!(a.manhattan_distance(&b), 7);
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

        let l1 = n.level_suppression(1);
        let l2 = n.level_suppression(2);
        let l3 = n.level_suppression(3);
        let l4 = n.level_suppression(4);
        let l5 = n.level_suppression(5);

        assert_eq!(l1, 1.0, "level 1 should always be 1.0");
        // Physiological satisfaction near-zero → levels 2+ heavily suppressed
        assert!(l2 < 0.1, "level 2 should be heavily suppressed, got {l2}");
        assert!(l3 < 0.1, "level 3 should be heavily suppressed, got {l3}");
        assert!(l4 < 0.1, "level 4 should be heavily suppressed, got {l4}");
        assert!(l5 < 0.1, "level 5 should be heavily suppressed, got {l5}");

        // Each higher level ≤ the one below (monotone)
        assert!(l2 >= l3);
        assert!(l3 >= l4);
        assert!(l4 >= l5);
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

        let l5 = n.level_suppression(5);
        assert!(
            l5 > 0.7,
            "well-fed cat's level 5 should be mostly unsuppressed, got {l5}"
        );
    }
}
