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
            Self::ShadowFox => 0.25,
        }
    }

    /// Default defense value for this species.
    pub fn default_defense(self) -> f32 {
        match self {
            Self::Fox => 0.15,
            Self::Hawk => 0.05,
            Self::Snake => 0.10,
            Self::ShadowFox => 0.20,
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
            Self::Fox => 5,
            Self::Hawk => 3,
            Self::Snake => 4,
            Self::ShadowFox => 2,
        }
    }

    /// Per-tick spawn probability at map edges.
    pub fn spawn_chance(self) -> f32 {
        match self {
            Self::Fox => 0.001,
            Self::Hawk => 0.0005,
            Self::Snake => 0.0008,
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
}

impl WildAnimal {
    /// Create a new wild animal with species defaults.
    pub fn new(species: WildSpecies) -> Self {
        Self {
            species,
            behavior: species.default_behavior(),
            threat_power: species.default_threat_power(),
            defense: species.default_defense(),
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
    Circling { center_x: i32, center_y: i32, angle: f32 },
    /// Ambush: stationary, waiting.
    Waiting,
    /// Fleeing toward map edge after losing a fight.
    Fleeing { dx: i32, dy: i32 },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_defaults_are_consistent() {
        for species in [WildSpecies::Fox, WildSpecies::Hawk, WildSpecies::Snake, WildSpecies::ShadowFox] {
            let animal = WildAnimal::new(species);
            assert_eq!(animal.species, species);
            assert_eq!(animal.behavior, species.default_behavior());
            assert!(animal.threat_power > 0.0);
            assert!(animal.defense >= 0.0);
        }
    }

    #[test]
    fn fox_is_strongest_threat() {
        assert!(WildSpecies::ShadowFox.default_threat_power() > WildSpecies::Fox.default_threat_power());
        assert!(WildSpecies::Fox.default_threat_power() > WildSpecies::Hawk.default_threat_power());
        assert!(WildSpecies::Hawk.default_threat_power() > WildSpecies::Snake.default_threat_power());
    }

    #[test]
    fn population_caps_are_positive() {
        for species in [WildSpecies::Fox, WildSpecies::Hawk, WildSpecies::Snake, WildSpecies::ShadowFox] {
            assert!(species.population_cap() > 0);
        }
    }
}
