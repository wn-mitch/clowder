use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Wildlife species and behavior
// ---------------------------------------------------------------------------

/// The species of a wild animal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WildSpecies {
    Fox,
    Hawk,
    Snake,
    ShadowFox,
}

impl WildSpecies {
    /// Display name for narrative output.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fox => "fox",
            Self::Hawk => "hawk",
            Self::Snake => "snake",
            Self::ShadowFox => "shadow-fox",
        }
    }

    /// Single-character symbol for the TUI map.
    pub fn symbol(self) -> char {
        match self {
            Self::Fox => 'f',
            Self::Hawk => 'h',
            Self::Snake => 's',
            Self::ShadowFox => 'F',
        }
    }

    /// Default threat power for this species.
    pub fn default_threat_power(self) -> f32 {
        match self {
            Self::Fox => 0.15,
            Self::Hawk => 0.10,
            Self::Snake => 0.08,
            Self::ShadowFox => 0.18,
        }
    }

    /// Default defense value for this species.
    pub fn default_defense(self) -> f32 {
        match self {
            Self::Fox => 0.15,
            Self::Hawk => 0.05,
            Self::Snake => 0.10,
            // Shadow-foxes are spectral and fragile — they rely on
            // ambush-terror for their damage, not armor. A posse of cats
            // can meaningfully harm one without needing elite combat
            // training. See assets/narrative/banishment.ron.
            Self::ShadowFox => 0.08,
        }
    }

    /// Default behavior pattern for this species.
    pub fn default_behavior(self) -> BehaviorType {
        match self {
            Self::Fox => BehaviorType::Patrol,
            Self::Hawk => BehaviorType::Circle,
            Self::Snake => BehaviorType::Ambush,
            Self::ShadowFox => BehaviorType::Patrol,
        }
    }

    /// Maximum population cap for runtime spawning.
    pub fn population_cap(self) -> usize {
        match self {
            Self::Fox => 7,
            Self::Hawk => 5,
            Self::Snake => 5,
            Self::ShadowFox => 2,
        }
    }

    /// Per-tick spawn probability at map edges.
    pub fn spawn_chance(self) -> f32 {
        match self {
            Self::Fox => 0.003,
            Self::Hawk => 0.002,
            Self::Snake => 0.002,
            Self::ShadowFox => 0.0, // corruption-spawned only, not edge-spawned
        }
    }
}

/// How a wild animal moves and hunts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BehaviorType {
    /// Walk along terrain edges (fox).
    Patrol,
    /// Circle around a center point (hawk).
    Circle,
    /// Stay still, strike when prey is adjacent (snake).
    Ambush,
}

// ---------------------------------------------------------------------------
// WildAnimal component
// ---------------------------------------------------------------------------

/// Marks an entity as a wild animal with species-specific behavior.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WildAnimal {
    pub species: WildSpecies,
    pub behavior: BehaviorType,
    pub threat_power: f32,
    pub defense: f32,
    /// Ticks remaining before this animal can initiate a new stalk after ambushing.
    pub ambush_cooldown: u32,
}

impl WildAnimal {
    /// Create a new wild animal with species defaults.
    pub fn new(species: WildSpecies) -> Self {
        Self {
            species,
            behavior: species.default_behavior(),
            threat_power: species.default_threat_power(),
            defense: species.default_defense(),
            ambush_cooldown: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// WildlifeAiState — per-entity behavior state
// ---------------------------------------------------------------------------

/// Mutable AI state for wildlife movement decisions.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WildlifeAiState {
    /// Patrol: current direction of travel along terrain edge.
    Patrolling { dx: i32, dy: i32 },
    /// Circle: center point and current angle (radians).
    Circling {
        center_x: i32,
        center_y: i32,
        angle: f32,
    },
    /// Ambush: stationary, waiting.
    Waiting,
    /// Fleeing toward map edge after losing a fight.
    Fleeing { dx: i32, dy: i32 },
    /// Stalking: moving toward a cat to ambush it.
    Stalking { target_x: i32, target_y: i32 },
    /// Encircling a ward — shadow fox deposits corruption to siege it.
    EncirclingWard {
        ward_x: i32,
        ward_y: i32,
        angle: f32,
        ticks: u64,
    },
}

// ---------------------------------------------------------------------------
// Carcass — left behind by shadow fox kills
// ---------------------------------------------------------------------------

use crate::components::prey::PreyKind;

/// A rotting carcass left by a shadow fox kill. Emits corruption unless cleansed.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Carcass {
    pub prey_kind: PreyKind,
    pub age_ticks: u64,
    pub corruption_rate: f32,
    pub cleansed: bool,
    pub harvested: bool,
}

// ---------------------------------------------------------------------------
// Fox ecology — per-entity state, lifecycle, AI phase, and dens
// ---------------------------------------------------------------------------

/// Sex of a fox, used for pairing and breeding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoxSex {
    Male,
    Female,
}

/// Life stage of a fox. Determines available behaviors and mortality curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoxLifeStage {
    /// 0–1 season. Stays at den. Cannot hunt. Vulnerable to predators.
    Cub,
    /// 1–3 seasons. Disperses from natal den seeking unclaimed territory.
    Juvenile,
    /// 3–16 seasons. Full capabilities. Breeds during winter.
    Adult,
    /// 16+ seasons. Declining health, increasing mortality.
    Elder,
}

/// High-level behavioral phase for fox AI decision-making.
///
/// This sits above `WildlifeAiState` (which handles physical movement).
/// `fox_ai_decision` sets both the phase and the corresponding movement state.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FoxAiPhase {
    /// Default: patrol territory perimeter.
    PatrolTerritory { dx: i32, dy: i32 },
    /// Actively seeking small prey (mice, rats, rabbits — NOT cats).
    HuntingPrey { target: Option<u64> },
    /// Heading back to den after a hunt.
    Returning { x: i32, y: i32 },
    /// At den, resting/digesting. Well-fed foxes spend most time here.
    Resting { ticks: u64 },
    /// Juvenile looking for unclaimed territory.
    Dispersing { dx: i32, dy: i32 },
    /// Depositing scent marks at territory boundary.
    ScentMarking,
    /// In a standoff with a cat or rival fox.
    Confronting {
        target_id: u64,
        ticks_remaining: u64,
    },
    /// Retreating from danger toward map edge.
    Fleeing { dx: i32, dy: i32 },
    /// Approaching colony stores to steal food.
    Raiding { target_x: i32, target_y: i32 },
    /// Staying near den with cubs present.
    DenGuarding,
}

/// Per-fox mutable state: needs, lifecycle, and territory association.
///
/// Attached alongside `WildAnimal` to distinguish foxes from other wildlife.
/// Systems query `With<FoxState>` for fox-specific behavior.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxState {
    /// 0.0 = full, 1.0 = starving. Decays toward 1.0 each tick unless satiated.
    pub hunger: f32,
    /// Ticks remaining before hunger resumes decaying. Set on successful hunt/raid.
    pub satiation_ticks: u64,
    /// Current life stage.
    pub life_stage: FoxLifeStage,
    /// Ticks since birth.
    pub age_ticks: u64,
    /// Biological sex.
    pub sex: FoxSex,
    /// Associated den entity. None for dispersing juveniles.
    pub home_den: Option<Entity>,
    /// Paired mate entity.
    pub mate: Option<Entity>,
    /// Ticks before next hunt/confrontation attempt.
    pub post_action_cooldown: u64,
    /// 0.0–1.0. Derived from hunger — starving foxes are bold.
    pub boldness: f32,
    /// Consecutive ticks with `hunger >= 1.0`. Resets to 0 when satiated.
    /// Foxes die when this exceeds `fc.starvation_death_ticks`.
    pub starvation_ticks: u64,
    /// Tick when this fox last completed a patrol (DepositScent). Used by
    /// scoring to build pressure for periodic patrolling even when hunger wins.
    pub last_patrol_tick: u64,
}

impl FoxState {
    /// Create a new adult fox.
    pub fn new_adult(sex: FoxSex, den: Option<Entity>) -> Self {
        Self {
            hunger: 0.5,
            satiation_ticks: 0,
            life_stage: FoxLifeStage::Adult,
            age_ticks: 60_000, // ~3 seasons old
            sex,
            home_den: den,
            mate: None,
            post_action_cooldown: 0,
            boldness: 0.25,
            starvation_ticks: 0,
            last_patrol_tick: 0,
        }
    }

    /// Create a new cub at a den.
    pub fn new_cub(sex: FoxSex, den: Entity) -> Self {
        Self {
            hunger: 0.0,
            satiation_ticks: 8000, // cubs are nursed initially; safety buffer
            life_stage: FoxLifeStage::Cub,
            age_ticks: 0,
            sex,
            home_den: Some(den),
            mate: None,
            post_action_cooldown: 0,
            boldness: 0.0,
            starvation_ticks: 0,
            last_patrol_tick: 0,
        }
    }
}

/// A fox den — territory anchor and breeding site.
///
/// Follows `PreyDen` pattern. Each den represents a mated pair's home base.
/// Territory extends `territory_radius` tiles from the den position.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxDen {
    /// How far the territory extends from this den.
    pub territory_radius: i32,
    /// Number of living cubs at this den.
    pub cubs_present: u32,
    /// Scent strength at this den (0.0–1.0). Refreshed by patrolling adults.
    pub scent_strength: f32,
    /// Tick when this den was established.
    pub established_tick: u64,
    /// Tick of the last successful `FeedCubs` resolution. Defaults to 0
    /// (never fed). Used by `feed_cubs_at_dens` to refresh cub satiation.
    pub last_fed_tick: u64,
}

impl FoxDen {
    pub fn new(territory_radius: i32, tick: u64) -> Self {
        Self {
            territory_radius,
            cubs_present: 0,
            scent_strength: 0.5,
            established_tick: tick,
            last_fed_tick: tick, // treat spawn as freshly fed
        }
    }
}

// ---------------------------------------------------------------------------
// ActiveConfrontation — shared state for paired standoffs
// ---------------------------------------------------------------------------

/// Role in an active confrontation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfrontationRole {
    /// Initiated the confrontation (e.g., fox defending den).
    Attacker,
    /// Was confronted (e.g., cat that strayed too close).
    Defender,
}

/// Why the confrontation started. Drives escalation chance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfrontationReason {
    /// Fox is defending cubs at its den — high stakes, high escalation.
    DenDefense,
    /// Fox is starving and attacked a vulnerable cat.
    DesperateAttack,
    /// Territory dispute (future: two foxes).
    TerritoryDispute,
}

/// Shared state for a paired confrontation between two entities (fox vs cat
/// or fox vs fox). Inserted on BOTH participants so each side's AI sees the
/// encounter and can decide fight-or-flight independently.
///
/// The `min_commitment` field prevents oscillation: once locked in, neither
/// side can disengage for at least this many ticks.
#[derive(Component, Debug, Clone)]
pub struct ActiveConfrontation {
    pub partner: Entity,
    pub role: ConfrontationRole,
    pub reason: ConfrontationReason,
    pub ticks_remaining: u64,
    pub min_commitment: u64,
    /// Tick when the confrontation started — used to enforce min_commitment.
    pub started_tick: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_defaults_are_consistent() {
        for species in [
            WildSpecies::Fox,
            WildSpecies::Hawk,
            WildSpecies::Snake,
            WildSpecies::ShadowFox,
        ] {
            let animal = WildAnimal::new(species);
            assert_eq!(animal.species, species);
            assert_eq!(animal.behavior, species.default_behavior());
            assert!(animal.threat_power > 0.0);
            assert!(animal.defense >= 0.0);
        }
    }

    #[test]
    fn fox_is_strongest_threat() {
        assert!(
            WildSpecies::ShadowFox.default_threat_power() > WildSpecies::Fox.default_threat_power()
        );
        assert!(WildSpecies::Fox.default_threat_power() > WildSpecies::Hawk.default_threat_power());
        assert!(
            WildSpecies::Hawk.default_threat_power() > WildSpecies::Snake.default_threat_power()
        );
    }

    #[test]
    fn population_caps_are_positive() {
        for species in [
            WildSpecies::Fox,
            WildSpecies::Hawk,
            WildSpecies::Snake,
            WildSpecies::ShadowFox,
        ] {
            assert!(species.population_cap() > 0);
        }
    }

    #[test]
    fn fox_state_new_adult() {
        let state = FoxState::new_adult(FoxSex::Female, None);
        assert_eq!(state.life_stage, FoxLifeStage::Adult);
        assert!((state.hunger - 0.5).abs() < f32::EPSILON);
        assert_eq!(state.satiation_ticks, 0);
        assert!(state.home_den.is_none());
    }

    #[test]
    fn fox_state_new_cub() {
        let den = Entity::from_bits(42);
        let state = FoxState::new_cub(FoxSex::Male, den);
        assert_eq!(state.life_stage, FoxLifeStage::Cub);
        assert_eq!(state.hunger, 0.0);
        assert_eq!(state.home_den, Some(den));
        assert_eq!(state.boldness, 0.0);
    }

    #[test]
    fn fox_den_defaults() {
        let den = FoxDen::new(18, 100);
        assert_eq!(den.territory_radius, 18);
        assert_eq!(den.cubs_present, 0);
        assert!(den.scent_strength > 0.0);
    }
}
