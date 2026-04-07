use bevy_ecs::prelude::*;

use crate::components::items::ItemKind;
use crate::resources::map::Terrain;

// ---------------------------------------------------------------------------
// PreySpecies
// ---------------------------------------------------------------------------

/// The species of a huntable prey animal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PreySpecies {
    Mouse,
    Rat,
    Fish,
    Bird,
}

impl PreySpecies {
    /// Single-character symbol for the TUI map.
    pub fn symbol(self) -> char {
        match self {
            Self::Mouse => 'm',
            Self::Rat => 'r',
            Self::Fish => '~',
            Self::Bird => 'b',
        }
    }

    /// Per-tick probability that a new individual is added to the population.
    pub fn breed_rate(self) -> f32 {
        match self {
            Self::Mouse => 0.003,
            Self::Rat => 0.005,
            Self::Fish => 0.002,
            Self::Bird => 0.001,
        }
    }

    /// Maximum number of individuals of this species allowed in the world.
    pub fn population_cap(self) -> usize {
        match self {
            Self::Mouse => 30,
            Self::Rat => 50,
            Self::Fish => 20,
            Self::Bird => 15,
        }
    }

    /// Terrain types where this species can spawn and live.
    pub fn habitat(self) -> &'static [Terrain] {
        match self {
            Self::Mouse => &[Terrain::Grass, Terrain::LightForest],
            Self::Rat => &[Terrain::Grass, Terrain::LightForest, Terrain::DenseForest],
            Self::Fish => &[Terrain::Water],
            Self::Bird => &[Terrain::Grass, Terrain::LightForest],
        }
    }

    /// The item dropped when this animal is caught.
    pub fn item_kind(self) -> ItemKind {
        match self {
            Self::Mouse => ItemKind::RawMouse,
            Self::Rat => ItemKind::RawRat,
            Self::Fish => ItemKind::RawFish,
            Self::Bird => ItemKind::RawBird,
        }
    }

    /// Display name for narrative output.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::Rat => "rat",
            Self::Fish => "fish",
            Self::Bird => "bird",
        }
    }
}

// ---------------------------------------------------------------------------
// PreyAnimal component
// ---------------------------------------------------------------------------

/// Marks an entity as a huntable prey animal.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyAnimal {
    pub species: PreySpecies,
    /// Hunger level: 0.0 = full, 1.0 = starving.
    pub hunger: f32,
}

impl PreyAnimal {
    /// Create a new, well-fed prey animal of the given species.
    pub fn new(species: PreySpecies) -> Self {
        Self {
            species,
            hunger: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prey_maps_to_correct_item() {
        assert_eq!(PreySpecies::Mouse.item_kind(), ItemKind::RawMouse);
        assert_eq!(PreySpecies::Rat.item_kind(), ItemKind::RawRat);
        assert_eq!(PreySpecies::Fish.item_kind(), ItemKind::RawFish);
        assert_eq!(PreySpecies::Bird.item_kind(), ItemKind::RawBird);
    }

    #[test]
    fn population_caps_are_reasonable() {
        assert!(
            PreySpecies::Rat.population_cap() > PreySpecies::Mouse.population_cap(),
            "Rat cap should exceed Mouse cap"
        );
        assert!(
            PreySpecies::Bird.population_cap() < PreySpecies::Mouse.population_cap(),
            "Bird cap should be below Mouse cap"
        );
    }

    #[test]
    fn new_prey_animal_starts_full() {
        for species in [
            PreySpecies::Mouse,
            PreySpecies::Rat,
            PreySpecies::Fish,
            PreySpecies::Bird,
        ] {
            let animal = PreyAnimal::new(species);
            assert_eq!(animal.species, species);
            assert_eq!(animal.hunger, 0.0, "{} should start with hunger 0.0", species.name());
        }
    }

    #[test]
    fn habitat_is_non_empty() {
        for species in [
            PreySpecies::Mouse,
            PreySpecies::Rat,
            PreySpecies::Fish,
            PreySpecies::Bird,
        ] {
            assert!(
                !species.habitat().is_empty(),
                "{} must have at least one habitat terrain",
                species.name()
            );
        }
    }
}
