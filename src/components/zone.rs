use bevy_ecs::prelude::*;

/// A player-placed zone marker that mildly nudges cat behavior.
///
/// Zone markers are not commands — they're suggestions that weight the
/// utility AI scoring. A cat near a BuildHere zone gets +0.1 to Build.
#[derive(Component, Debug, Clone)]
pub struct Zone {
    pub kind: ZoneKind,
}

/// The type of zone designation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    /// Cats get a mild bonus to Build actions near this zone.
    BuildHere,
    /// Cats get a mild bonus to Farm actions near this zone.
    FarmHere,
    /// Cats avoid Wander/Explore toward this area.
    Avoid,
}

impl ZoneKind {
    pub fn cycle(self) -> Self {
        match self {
            Self::BuildHere => Self::FarmHere,
            Self::FarmHere => Self::Avoid,
            Self::Avoid => Self::BuildHere,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BuildHere => "Build",
            Self::FarmHere => "Farm",
            Self::Avoid => "Avoid",
        }
    }

    pub fn symbol(self) -> char {
        match self {
            Self::BuildHere => 'B',
            Self::FarmHere => 'F',
            Self::Avoid => '!',
        }
    }
}
