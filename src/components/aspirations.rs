use bevy_ecs::prelude::*;

use crate::ai::Action;

// ---------------------------------------------------------------------------
// Aspiration Domain
// ---------------------------------------------------------------------------

/// The broad domains that aspirations can belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AspirationDomain {
    Hunting,
    Combat,
    Social,
    Herbcraft,
    Exploration,
    Building,
    Leadership,
}

impl AspirationDomain {
    /// Actions that fall under this domain, used for desire bonuses.
    pub fn matching_actions(self) -> &'static [Action] {
        match self {
            Self::Hunting => &[Action::Hunt, Action::Forage],
            Self::Combat => &[Action::Fight, Action::Patrol],
            Self::Social => &[Action::Socialize, Action::GroomOther],
            Self::Herbcraft => &[Action::Herbcraft],
            Self::Exploration => &[Action::Explore, Action::Wander],
            Self::Building => &[Action::Build],
            Self::Leadership => &[Action::Coordinate],
        }
    }
}

// ---------------------------------------------------------------------------
// Milestone conditions
// ---------------------------------------------------------------------------

/// Conditions that can gate milestone completion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MilestoneCondition {
    /// Accumulate N successful actions of a type.
    ActionCount { action: String, count: u32 },
    /// Reach a skill level threshold.
    SkillLevel { skill: String, level: f32 },
    /// Form a bond of a specific type.
    FormBond { bond_type: String },
    /// Witness N events of a specific type.
    WitnessEvent { event_type: String, count: u32 },
    /// Teach/mentor another cat N times.
    Mentor { count: u32 },
}

/// A single milestone within an aspiration chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Milestone {
    pub name: String,
    pub condition: MilestoneCondition,
    pub narrative_on_complete: String,
}

// ---------------------------------------------------------------------------
// Aspiration chain (data definition — loaded from RON)
// ---------------------------------------------------------------------------

/// A full aspiration chain loaded from RON data files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AspirationChain {
    pub name: String,
    pub domain: AspirationDomain,
    pub milestones: Vec<Milestone>,
    pub completion_narrative: String,
}

// ---------------------------------------------------------------------------
// Active aspiration (per-cat runtime state)
// ---------------------------------------------------------------------------

/// An aspiration a cat is actively pursuing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActiveAspiration {
    pub chain_name: String,
    pub domain: AspirationDomain,
    pub current_milestone: usize,
    /// Accumulated progress toward the current milestone's condition.
    pub progress: u32,
    /// Tick when this aspiration was adopted.
    pub adopted_tick: u64,
    /// Tick when progress last advanced (for abandonment check).
    pub last_progress_tick: u64,
}

// ---------------------------------------------------------------------------
// Aspirations component
// ---------------------------------------------------------------------------

/// Tracks a cat's active and completed aspirations.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Aspirations {
    pub active: Vec<ActiveAspiration>,
    pub completed: Vec<String>,
}

// ---------------------------------------------------------------------------
// Preferences (likes / dislikes)
// ---------------------------------------------------------------------------

/// Whether a cat likes or dislikes an activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Preference {
    Like,
    Dislike,
}

/// A cat's personal likes and dislikes for specific actions.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Preferences {
    pub action_preferences: Vec<(Action, Preference)>,
}

impl Preferences {
    /// Look up the preference for a given action, if any.
    pub fn get(&self, action: Action) -> Option<Preference> {
        self.action_preferences
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, p)| *p)
    }
}

// ---------------------------------------------------------------------------
// Marker component
// ---------------------------------------------------------------------------

/// Inserted after a cat's aspirations and preferences have been initialised.
/// Systems use `Without<AspirationsInitialized>` to detect cats needing setup.
#[derive(Component, Debug, Clone, Copy)]
pub struct AspirationsInitialized;
