//! `MisfireEffect` — emitted whenever `apply_misfire` resolves a magic
//! misfire outcome (Ticket 471). Carries the per-event identity (caster,
//! position, tick) plus the discriminant (`MisfireEffectKind`) so the
//! event-log replay and the festering-wound consumer (Ticket 472) can
//! reason about each misfire independently.
//!
//! Before 471, misfire effects mutated `Corruption.0` / `Health.current`
//! synthetically with no log emit. The (26,61) seed-42 death class needed
//! 90 minutes of `CatSnapshot` trail reconstruction because every
//! `CorruptionBacksplash` / `WoundTransfer` was structurally invisible.

use bevy_ecs::prelude::*;

use crate::components::magic::MisfireEffectKind;
use crate::components::physical::Position;

#[derive(Message, Debug, Clone, Copy)]
pub struct MisfireEffect {
    /// The caster who misfired.
    pub entity: Entity,
    /// Which misfire outcome resolved.
    pub kind: MisfireEffectKind,
    /// Position of the caster at misfire time (also the position the
    /// inverted-ward / location-reveal beacon spawns at, for those
    /// kinds).
    pub position: Position,
    pub tick: u64,
}
