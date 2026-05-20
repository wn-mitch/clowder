//! Cat movement events emitted by `emit_cat_moved_messages` whenever a
//! cat's `Position` changes between FixedUpdate ticks. Authored by ticket
//! 431 (the post-428 flamegraph found `passive_familiarity`'s O(N²) sweep
//! at 64.43% inclusive CPU — the dominant hot frame in the entire sim).
//!
//! Per the per-tick discipline doctrine codified in CLAUDE.md "ECS rules"
//! (default to event-driven, justify per-tick): pair-set membership for
//! passive familiarity changes ONLY when a cat moves, not every tick. The
//! consumer in Stage B is `update_near_pair_cache`, which drains the
//! moved cat's existing pairings and re-inserts pairs within range.
//! Future cache subscribers (per-cat path-cost cache, joint-intention
//! spatial gates) reuse the same signal.
//!
//! Determinism note: emitted in archetype/entity-iteration order. Stage B's
//! `NearPairCache` uses `BTreeMap` so the final cache state is order-
//! independent — see `src/resources/relationships.rs:55-63` for the same
//! load-bearing constraint on the underlying `Relationships` map.

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// A cat's `Position` changed between FixedUpdate ticks. Emitted by
/// `emit_cat_moved_messages` in `src/systems/cat_movement.rs`, which
/// tracks per-cat last-known positions in a `Local<HashMap<Entity,
/// Position>>` and writes one message per cat whose position differs.
///
/// **Not emitted** when a cat first appears in the cats query (bootstrap
/// tick — subscribers self-initialize from the cats query directly) or
/// when a cat's position is rewritten to the same tile.
#[derive(Message, Debug, Clone, Copy)]
pub struct CatMoved {
    pub entity: Entity,
    pub from: Position,
    pub to: Position,
}
