use bevy_ecs::prelude::*;

use crate::ai::Action;
use crate::components::building::StructureType;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// Coordinator marker
// ---------------------------------------------------------------------------

/// Marker component for cats who have emerged as colony coordinators through
/// social weight, diligence, and sociability. Evaluated every ~100 ticks.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Coordinator;

// ---------------------------------------------------------------------------
// Directives
// ---------------------------------------------------------------------------

/// What kind of colony need a directive addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DirectiveKind {
    Hunt,
    Forage,
    Build,
    Fight,
    Patrol,
    Herbcraft,
    SetWard,
}

impl DirectiveKind {
    /// Map a directive kind to the corresponding cat action.
    pub fn to_action(self) -> Action {
        match self {
            DirectiveKind::Hunt => Action::Hunt,
            DirectiveKind::Forage => Action::Forage,
            DirectiveKind::Build => Action::Build,
            DirectiveKind::Fight => Action::Fight,
            DirectiveKind::Patrol => Action::Patrol,
            DirectiveKind::Herbcraft => Action::Herbcraft,
            DirectiveKind::SetWard => Action::Herbcraft, // ward-setting is a herbcraft sub-mode
        }
    }
}

/// A single directive produced by colony assessment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Directive {
    pub kind: DirectiveKind,
    /// Priority in [0.0, 1.0] — higher means more urgent.
    pub priority: f32,
    /// Suggested target entity (e.g. the damaged building, the injured cat).
    #[serde(skip)]
    pub target_entity: Option<Entity>,
    /// Suggested target position.
    pub target_position: Option<Position>,
    /// Blueprint for new construction (None = repair existing building).
    pub blueprint: Option<StructureType>,
}

/// Queue of pending directives on a coordinator entity.
/// Rebuilt every ~20 ticks by `assess_colony_needs`, consumed one at a time
/// as the coordinator walks to cats and delivers them.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DirectiveQueue {
    pub directives: Vec<Directive>,
}

// ---------------------------------------------------------------------------
// Directive delivery
// ---------------------------------------------------------------------------

/// Component placed on a target cat when a coordinator delivers a directive.
/// Provides a score bonus to the directed action at next evaluation.
#[derive(Component, Debug, Clone)]
pub struct ActiveDirective {
    /// The action the cat should perform.
    pub kind: DirectiveKind,
    /// Priority of the directive.
    pub priority: f32,
    /// Entity of the coordinator who issued this.
    pub coordinator: Entity,
    /// Coordinator's social weight at time of delivery.
    pub coordinator_social_weight: f32,
    /// Tick when this directive was delivered. Expires after ~200 ticks.
    pub delivered_tick: u64,
}

/// Directive-in-transit on a coordinator walking to deliver it.
/// Inserted when `Action::Coordinate` is chosen, removed on delivery.
#[derive(Component, Debug, Clone)]
pub struct PendingDelivery(pub Directive);

// ---------------------------------------------------------------------------
// Flag resource
// ---------------------------------------------------------------------------

/// Inserted when a coordinator dies, triggering immediate re-evaluation.
#[derive(Resource, Default)]
pub struct CoordinatorDied;

// ---------------------------------------------------------------------------
// Build pressure
// ---------------------------------------------------------------------------

/// Slowly-accumulating pressure channels that track unmet colony infrastructure
/// needs. Attached to coordinators. Each channel rises when its signal persists
/// and decays when it doesn't. The coordinator's attentiveness (derived from
/// personality) determines accumulation rate and action threshold.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BuildPressure {
    /// Stores at capacity for extended period.
    pub storage: f32,
    /// Cats sleeping outdoors (no Den in range).
    pub shelter: f32,
    /// Low social satisfaction despite Hearth.
    pub gathering: f32,
    /// Skilled crafters with no Workshop available.
    pub workshop: f32,
    /// Food scarcity with no Garden.
    pub farming: f32,
    /// Wildlife breaching colony perimeter.
    pub defense: f32,
}

impl BuildPressure {
    /// Pressure accumulation base rate per evaluation cycle.
    pub const BASE_RATE: f32 = 0.01;
    /// Decay factor applied when the signal is inactive.
    pub const DECAY: f32 = 0.95;

    /// The structure type each pressure channel corresponds to.
    pub fn highest_actionable(&self, threshold: f32) -> Option<StructureType> {
        let channels = [
            (self.shelter, StructureType::Den),
            (self.storage, StructureType::Stores),
            (self.gathering, StructureType::Hearth),
            (self.workshop, StructureType::Workshop),
            (self.farming, StructureType::Garden),
            (self.defense, StructureType::Watchtower),
        ];
        channels
            .iter()
            .filter(|(pressure, _)| *pressure > threshold)
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, kind)| *kind)
    }
}
