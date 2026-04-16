use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::items::ItemKind;
use crate::components::physical::{Health, Position};
use crate::resources::map::Terrain;

// ---------------------------------------------------------------------------
// PreyKind — thin identity enum
// ---------------------------------------------------------------------------

/// Species identity tag. Used for population counting, narrative, and UI.
/// All behavioral parameters live in `PreyConfig` (populated from the
/// `PreyProfile` trait at spawn time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PreyKind {
    Mouse,
    Rat,
    Rabbit,
    Fish,
    Bird,
}

// ---------------------------------------------------------------------------
// FleeStrategy
// ---------------------------------------------------------------------------

/// How a prey species flees from threats. Dispatched in the flee system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleeStrategy {
    /// Move directly away from threat at flee_speed tiles/tick.
    Standard,
    /// Like Standard, but bias toward LightForest/DenseForest tiles.
    SeekCover,
    /// Jump to a random passable tile 5-8 tiles from threat (birds).
    Teleport,
    /// Don't flee — return to Idle immediately (fish).
    Stationary,
}

// ---------------------------------------------------------------------------
// PreyAiState
// ---------------------------------------------------------------------------

/// AI state machine for prey animals.
#[derive(Debug, Clone, Copy, Default)]
pub enum PreyAiState {
    #[default]
    Idle,
    Grazing {
        dx: i32,
        dy: i32,
        ticks: u64,
    },
    /// Prey has detected a threat and freezes, watching. Transitions to
    /// Fleeing after the species-specific freeze duration.
    Alert {
        threat: Entity,
        ticks: u64,
    },
    Fleeing {
        from: Entity,
        toward: Option<(i32, i32)>,
        ticks: u64,
    },
}

// ---------------------------------------------------------------------------
// PreyConfig — flat data component (immutable after spawn)
// ---------------------------------------------------------------------------

/// Species behavioral parameters, populated from `PreyProfile::to_config()`
/// at spawn time. Systems read these fields directly — no trait dispatch.
#[derive(Component, Debug, Clone)]
pub struct PreyConfig {
    pub kind: PreyKind,
    pub name: &'static str,
    pub item_kind: ItemKind,
    pub flee_speed: u32,
    pub graze_cadence: u64,
    pub alert_radius: i32,
    pub freeze_ticks: u64,
    pub catch_difficulty: f32,
    pub flee_strategy: FleeStrategy,
    pub flee_duration: u64,
    pub habitat: &'static [Terrain],
}

// ---------------------------------------------------------------------------
// PreyState — mutable per-entity state
// ---------------------------------------------------------------------------

/// Per-entity mutable state for a prey animal.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyState {
    /// 0.0 = full, 1.0 = starving.
    pub hunger: f32,
    /// Awareness level, accumulates over time. Affects detection probability.
    #[serde(skip, default)]
    pub alertness: f32,
    /// Current AI state. Skipped during serialization because `Alert` and
    /// `Fleeing` contain `Entity` handles that aren't stable across save/load.
    #[serde(skip, default)]
    pub ai_state: PreyAiState,
    /// The den this prey is associated with. Set at spawn, used for O(1)
    /// lookups of predation pressure, flee direction, and roaming limits.
    #[serde(skip, default)]
    pub home_den: Option<Entity>,
}

impl Default for PreyState {
    fn default() -> Self {
        Self {
            hunger: 0.0,
            alertness: 0.0,
            ai_state: PreyAiState::Idle,
            home_den: None,
        }
    }
}

// ---------------------------------------------------------------------------
// PreyAnimal — marker component
// ---------------------------------------------------------------------------

/// Marker component for prey entities. Kept so existing queries using
/// `With<PreyAnimal>` / `Without<PreyAnimal>` continue working.
#[derive(Component, Debug, Clone, Copy)]
pub struct PreyAnimal;

// ---------------------------------------------------------------------------
// PreyDen — den component
// ---------------------------------------------------------------------------

/// A prey den: a persistent geographic spawn point for a species. Produces
/// prey nearby and refills naturally over time. Dens can be raided (weakened)
/// by cats, and abandoned under sustained predation pressure. Orphaned prey
/// can adopt empty dens or found new ones.
#[derive(Component, Debug, Clone)]
pub struct PreyDen {
    pub kind: PreyKind,
    /// Maximum spawns this den can hold.
    pub capacity: u32,
    /// Current spawns available. Decrements when prey spawn, refills over time.
    pub spawns_remaining: u32,
    /// Predation pressure from nearby kills. 0.0 = safe, 1.0 = extreme danger.
    pub predation_pressure: f32,
    /// Ticks spent with predation_pressure > 0.7. At 3000+ the den is abandoned.
    pub stressed_ticks: u32,
    /// Cached from species profile for raid drops.
    pub item_kind: ItemKind,
    pub den_name: &'static str,
    pub raid_drop: u32,
}

impl PreyDen {
    pub fn from_profile(profile: &dyn crate::species::PreyProfile) -> Self {
        Self {
            kind: profile.kind(),
            capacity: profile.den_capacity(),
            spawns_remaining: profile.den_capacity(),
            predation_pressure: 0.0,
            stressed_ticks: 0,
            item_kind: profile.item_kind(),
            den_name: profile.den_name(),
            raid_drop: profile.den_raid_drop(),
        }
    }

    /// Simple constructor for tests and world gen where profile is already consumed.
    pub fn new(kind: PreyKind, capacity: u32) -> Self {
        // Default item/name/drop for backwards compat. Use from_profile for full data.
        Self {
            kind,
            capacity,
            spawns_remaining: capacity,
            predation_pressure: 0.0,
            stressed_ticks: 0,
            item_kind: ItemKind::RawMouse,
            den_name: "den",
            raid_drop: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Prey events and resources
// ---------------------------------------------------------------------------

/// Message fired when a prey entity is killed (by cat or wildlife predator).
/// Used by `update_den_pressure` to adjust nearby den predation_pressure.
#[derive(bevy_ecs::prelude::Message, Debug, Clone)]
pub struct PreyKilled {
    pub kind: PreyKind,
    pub position: Position,
}

/// Message sent when a cat raids a den. Processed by `apply_den_raids` which
/// mutates the den (reduces spawns, spikes pressure) and spawns food items.
#[derive(bevy_ecs::prelude::Message, Debug, Clone)]
pub struct DenRaided {
    pub den_entity: Entity,
    pub kills: u32,
    pub item_kind: ItemKind,
    pub position: Position,
    pub den_name: &'static str,
}

/// Per-species population density (pop / cap), updated once per tick in
/// `prey_population`. Read by the pounce formula for density-dependent
/// vulnerability.
#[derive(Resource, Default, Debug)]
pub struct PreyDensity(pub HashMap<PreyKind, f32>);

// ---------------------------------------------------------------------------
// Spawn helpers
// ---------------------------------------------------------------------------

/// Bundle of components for spawning a prey entity.
pub type PreyBundle = (PreyAnimal, PreyConfig, PreyState, Health);

/// Create a prey entity bundle from a species profile.
pub fn prey_bundle(profile: &dyn crate::species::PreyProfile) -> PreyBundle {
    (
        PreyAnimal,
        profile.to_config(),
        profile.to_state(),
        Health::default(),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::{self, PreyProfile};

    #[test]
    fn all_species_produce_valid_configs() {
        let registry = species::build_registry();
        for profile in &registry.profiles {
            let config = profile.to_config();
            assert!(!config.name.is_empty());
            assert!(config.graze_cadence > 0);
            assert!(config.catch_difficulty > 0.0);
            assert!(config.catch_difficulty <= 1.0);
        }
    }

    #[test]
    fn registry_covers_all_kinds() {
        let registry = species::build_registry();
        let kinds: Vec<PreyKind> = registry.profiles.iter().map(|p| p.kind()).collect();
        assert!(kinds.contains(&PreyKind::Mouse));
        assert!(kinds.contains(&PreyKind::Rat));
        assert!(kinds.contains(&PreyKind::Rabbit));
        assert!(kinds.contains(&PreyKind::Fish));
        assert!(kinds.contains(&PreyKind::Bird));
    }

    #[test]
    fn prey_state_defaults_to_idle_and_full() {
        let state = PreyState::default();
        assert_eq!(state.hunger, 0.0);
        assert_eq!(state.alertness, 0.0);
        assert!(matches!(state.ai_state, PreyAiState::Idle));
    }

    #[test]
    fn population_caps_are_reasonable() {
        let registry = species::build_registry();
        let mouse = registry.find(PreyKind::Mouse);
        let rat = registry.find(PreyKind::Rat);
        let bird = registry.find(PreyKind::Bird);
        assert!(
            mouse.population_cap() > rat.population_cap(),
            "Mouse cap should exceed Rat cap (mice are most abundant)"
        );
        assert!(
            bird.population_cap() < rat.population_cap(),
            "Bird cap should be below Rat cap"
        );
    }
}
