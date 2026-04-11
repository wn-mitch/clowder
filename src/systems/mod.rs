use bevy_ecs::prelude::*;

/// Bundles colony-wide optional resources that many scoring systems need.
/// Exists to keep systems under Bevy's 16-param limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct ColonyContext<'w> {
    pub knowledge: Option<Res<'w, crate::resources::colony_knowledge::ColonyKnowledge>>,
    pub priority: Option<Res<'w, crate::resources::colony_priority::ColonyPriority>>,
    pub exploration_map: Res<'w, crate::resources::ExplorationMap>,
    pub fox_scent_map: Res<'w, crate::resources::FoxScentMap>,
    pub cat_presence_map: ResMut<'w, crate::resources::CatPresenceMap>,
}

pub mod actions;
pub mod ai;
pub mod aspirations;
pub mod buildings;
pub mod colony_knowledge;
pub mod colony_score;
pub mod combat;
pub mod coordination;
pub mod death;
pub mod disposition;
pub mod fate;
pub mod goap;
pub mod growth;
pub mod items;
pub mod magic;
pub mod memory;
pub mod mood;
pub mod narrative;
pub mod needs;
pub mod personality_events;
pub mod personality_friction;
pub mod pregnancy;
pub mod prey;
pub mod snapshot;
pub mod social;
pub mod task_chains;
pub mod time;
pub mod weather;
pub mod wildlife;
pub mod wind;
