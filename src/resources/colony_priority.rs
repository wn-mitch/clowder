use bevy_ecs::prelude::*;

/// Colony-wide priority set by the player via the P key.
///
/// Applies a mild utility bonus (+0.15) to actions aligned with the priority,
/// nudging cat behavior without overriding critical needs.
#[derive(Resource, Debug, Default)]
pub struct ColonyPriority {
    pub active: Option<PriorityKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityKind {
    Food,
    Defense,
    Building,
    Exploration,
}

impl PriorityKind {
    pub fn cycle(current: Option<Self>) -> Option<Self> {
        match current {
            None => Some(Self::Food),
            Some(Self::Food) => Some(Self::Defense),
            Some(Self::Defense) => Some(Self::Building),
            Some(Self::Building) => Some(Self::Exploration),
            Some(Self::Exploration) => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Food => "Food",
            Self::Defense => "Defense",
            Self::Building => "Building",
            Self::Exploration => "Exploration",
        }
    }
}
