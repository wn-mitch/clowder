//! Per-cat cache of the most-recent `BodyPartInjury` event.
//!
//! Populated by [`cache_last_body_part_injury`] (an observer system in
//! `src/systems/injury_cache.rs`) that drains `MessageReader<BodyPartInjury>`
//! each tick. Read by [`check_death`] (ticket 471) to populate the
//! `EventKind::Death.injury_source` field with the source of the cat's
//! most recent wound — closing the `injury_source: null` gap that made
//! the seed-42 (26,61) death class diagnostically opaque.
//!
//! 095 Phase 1 Stage B retired the per-tick `Injury.source` history on
//! `Health`; this Component re-establishes the same information at a
//! sparse cadence (one update per body-part injury event), tied to the
//! cat's lifetime (auto-cleaned on despawn).

use bevy_ecs::prelude::*;

use crate::components::body_zones::BodyPart;
use crate::components::physical::InjurySource;

#[derive(Component, Debug, Clone, Copy)]
pub struct LastBodyPartInjury {
    pub source: InjurySource,
    pub part: BodyPart,
    pub tick: u64,
}
