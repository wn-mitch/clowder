pub mod pathfinding;
pub mod scoring;

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

/// The discrete actions available to a cat in Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Eat,
    Sleep,
    Wander,
    Idle,
}

// ---------------------------------------------------------------------------
// CurrentAction component
// ---------------------------------------------------------------------------

/// Tracks what a cat is currently doing and how long it will continue.
#[derive(Component, Debug, Clone)]
pub struct CurrentAction {
    pub action: Action,
    /// How many simulation ticks remain for this action.
    pub ticks_remaining: u64,
    /// Optional spatial target (e.g. food source, sleeping spot).
    pub target_position: Option<Position>,
}

impl Default for CurrentAction {
    fn default() -> Self {
        Self {
            action: Action::Idle,
            ticks_remaining: 0,
            target_position: None,
        }
    }
}
