pub mod fox_planner;
pub mod fox_scoring;
pub mod mating;
pub mod pathfinding;
pub mod planner;
pub mod scoring;

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

/// The discrete actions available to a cat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Eat,
    Sleep,
    Hunt,
    Forage,
    Wander,
    Idle,
    Socialize,
    Groom,
    Explore,
    Flee,
    Fight,
    Patrol,
    Build,
    Farm,
    Herbcraft,
    PracticeMagic,
    Coordinate,
    Mentor,
    Mate,
    Caretake,
    /// Prepare raw food at a Kitchen structure, transforming it into a cooked
    /// item that restores more hunger when eaten. Fulfillment-tier.
    Cook,
}

// ---------------------------------------------------------------------------
// CurrentAction component
// ---------------------------------------------------------------------------

/// Tracks what a cat is currently doing and how long it will continue.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentAction {
    pub action: Action,
    /// How many simulation ticks remain for this action.
    pub ticks_remaining: u64,
    /// Optional spatial target (e.g. food source, sleeping spot).
    pub target_position: Option<Position>,
    /// Optional entity target (e.g. cat to socialize/groom with).
    #[serde(skip)]
    pub target_entity: Option<Entity>,
    /// Top-3 action scores from the last decision (post-bonus, post-suppression).
    #[serde(skip)]
    pub last_scores: Vec<(Action, f32)>,
}

impl Default for CurrentAction {
    fn default() -> Self {
        Self {
            action: Action::Idle,
            ticks_remaining: 0,
            target_position: None,
            target_entity: None,
            last_scores: Vec::new(),
        }
    }
}
