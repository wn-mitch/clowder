use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// A cat snapped at a nearby cat due to unmet needs and high temper.
#[derive(Event)]
pub struct TemperFlared {
    pub cat: Entity,
    pub target: Option<Entity>,
}

/// A cat refused a coordinator directive due to stubbornness.
#[derive(Event)]
pub struct DirectiveRefused {
    pub cat: Entity,
    pub coordinator: Entity,
}

/// A playful cat initiated a group play session.
#[derive(Event)]
pub struct PlayInitiated {
    pub cat: Entity,
}

/// A proud cat's respect dropped critically low, triggering status-seeking.
#[derive(Event)]
pub struct PrideCrisis {
    pub cat: Entity,
}

/// An ambitious cat challenged the current coordinator's authority.
#[derive(Event)]
pub struct LeadershipChallenge {
    pub challenger: Entity,
    pub coordinator: Entity,
}

/// A traditional cat was forced away from familiar territory.
#[derive(Event)]
pub struct TraditionBroken {
    pub cat: Entity,
    pub location: Position,
}

/// An independent cat conspicuously broke away from group activity.
#[derive(Event)]
pub struct WentSolo {
    pub cat: Entity,
}
