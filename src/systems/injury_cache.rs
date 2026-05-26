//! Ticket 471 — per-cat last-injury cache observer.
//!
//! Drains [`BodyPartInjury`] each tick and writes the most-recent injury's
//! source/part/tick to a per-entity [`LastBodyPartInjury`] component. The
//! death discriminator (`check_death` in `src/systems/death.rs`) reads
//! this cache to populate `EventKind::Death.injury_source` instead of the
//! hardcoded `None` that 095 Phase 1 Stage B's retirement of
//! `Health.injuries` left behind.
//!
//! Two design notes:
//!
//! 1. **Component, not resource.** Cache lifetime is bound to the cat
//!    via the Component lifecycle, so despawn auto-cleans the entry. A
//!    global `HashMap<Entity, ...>` resource would orphan keys on
//!    despawn and require manual eviction in `check_death`.
//! 2. **Component, not body-model field.** 095 deliberately retired the
//!    per-part `last_injury_source` storage because the body model is
//!    queried at high cadence by combat / herbcraft / mobility reads.
//!    Tucking this away on a sparse Component keeps the body model
//!    lean.

use bevy_ecs::prelude::*;

use crate::components::injury_cache::LastBodyPartInjury;
use crate::messages::body_part_injury::BodyPartInjury;

/// Drains pending [`BodyPartInjury`] messages and writes the most recent
/// per-entity event to [`LastBodyPartInjury`].
pub fn cache_last_body_part_injury(
    mut commands: Commands,
    mut reader: MessageReader<BodyPartInjury>,
) {
    for msg in reader.read() {
        commands.entity(msg.entity).insert(LastBodyPartInjury {
            source: msg.source,
            part: msg.part,
            tick: msg.tick,
        });
    }
}
