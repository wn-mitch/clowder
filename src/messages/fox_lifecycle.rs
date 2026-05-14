//! Fox-side lifecycle events. These are *world-state* messages emitted
//! by `wildlife.rs`'s fox lifecycle systems and consumed by the §4 fox
//! marker authors (ticket 050).
//!
//! Distinct from [`WitnessableEvent`](super::witnessable_event::WitnessableEvent),
//! which carries *perceivable* events that drive cat mental-model
//! updates — these messages are mechanical fox bookkeeping (who claimed
//! which den, when a litter was born) with no observer-side belief
//! semantics.

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// A fox's home_den just transitioned from `None` → `Some(den)`. Emitted
/// at every fox spawn site that assigns a den (initial pair spawn,
/// cub birth) and at any runtime claim. Consumed by
/// `fox_spatial::update_den_marker` to insert the `HasDen` marker.
#[derive(Message, Debug, Clone, Copy)]
pub struct DenClaimed {
    pub fox: Entity,
    pub den: Entity,
    pub position: Position,
    pub tick: u64,
}

/// Why a fox lost its home_den.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenLostReason {
    /// Cub matured to juvenile and dispersed from its birth den.
    Maturation,
    /// Fox died while holding a den.
    Death,
    /// Fox abandoned the den voluntarily (no current emitter — reserved
    /// for future eviction / displacement systems).
    Abandoned,
}

/// A fox's home_den just transitioned from `Some(den)` → `None`. Emitted
/// at every site that clears `fox_state.home_den`. Consumed by
/// `fox_spatial::update_den_marker` to remove the `HasDen` marker.
#[derive(Message, Debug, Clone, Copy)]
pub struct DenLost {
    pub fox: Entity,
    pub den: Entity,
    pub reason: DenLostReason,
    pub tick: u64,
}

/// A litter just spawned at a fox den. Emitted by `breed_at_dens` right
/// after `den.cubs_present` is bumped from 0 → `litter_size`. Consumed
/// by `fox_spatial::update_cub_marker` to insert the `HasCubs` marker
/// on the mother fox.
///
/// **Cleanup note** — `HasCubs` is removed when `den.cubs_present`
/// transitions back to 0 (cub matured / cub died). The event-driven
/// cleanup path needs a mother-of-cub linkage that the data model
/// doesn't carry today; until that lands the per-tick scan in
/// `update_cub_marker` covers removal. See the 050 Log for the
/// follow-on.
#[derive(Message, Debug, Clone, Copy)]
pub struct CubsBorn {
    pub mother: Entity,
    pub den: Entity,
    pub count: u32,
    pub position: Position,
    pub tick: u64,
}
