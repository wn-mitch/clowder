//! Emits `CatMoved` messages whenever a cat's `Position` changes between
//! FixedUpdate ticks. Authored by ticket 431 Stage A as the substrate for
//! event-driven cache invalidation across multiple consumers (passive
//! familiarity pair-set cache in Stage B, per-cat path-cost cache in Stage C).
//!
//! Per the per-tick discipline doctrine codified in CLAUDE.md "ECS rules":
//! pair-set membership and per-cat path-cost fields change ONLY when a cat
//! moves, not every tick. This emit system runs once per FixedUpdate AFTER
//! every cat-stepping resolver (resolve_goap_plans, resolve_disposition_chains,
//! resolve_task_chains, resolve_magic_task_chains) so a single centralized
//! signal captures every Position mutation regardless of which resolver
//! caused it.
//!
//! Determinism note: the `Local<HashMap<Entity, Position>>` is never
//! iterated for output — only point-queried via `get(&entity)` from the
//! deterministic `cats.iter()` loop. Messages are written in archetype/
//! entity-iteration order. Downstream consumers (e.g. `NearPairCache`
//! built as a `BTreeMap`) must keep their final state order-independent.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;

use crate::components::building::Structure;
use crate::components::physical::{Dead, Position};
use crate::messages::cat_moved::CatMoved;

/// Detects cat Position deltas since the last FixedUpdate tick and writes
/// one `CatMoved` message per cat whose tile changed. Skips no-op writes
/// (same-cell rewrites) and bootstrap ticks (first sighting after spawn —
/// subscribers self-initialize from the cats query directly).
///
/// The `Local<HashMap<Entity, Position>>` is private system state; despawn
/// pruning happens at the end of each tick so the map size tracks live
/// cat count, not historical churn.
#[allow(clippy::type_complexity)]
pub fn emit_cat_moved_messages(
    mut last_positions: Local<HashMap<Entity, Position>>,
    cats: Query<(Entity, &Position), (Without<Dead>, Without<Structure>)>,
    mut writer: MessageWriter<CatMoved>,
) {
    let mut seen: HashSet<Entity> = HashSet::with_capacity(last_positions.len());
    for (entity, pos) in cats.iter() {
        seen.insert(entity);
        let cur = *pos;
        if let Some(&prev) = last_positions.get(&entity) {
            if prev != cur {
                writer.write(CatMoved {
                    entity,
                    from: prev,
                    to: cur,
                });
            }
        }
        last_positions.insert(entity, cur);
    }
    last_positions.retain(|e, _| seen.contains(e));
}
