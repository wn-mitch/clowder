//! Event-driven cache of cat pairs within `passive_familiarity_range` of
//! each other. Authored by ticket 431 Stage B to retire the per-tick O(N²)
//! pair sweep in [`passive_familiarity`](crate::systems::social::passive_familiarity),
//! the dominant hot frame in the 2026-05-20 baseline flamegraph at 64.43%
//! inclusive CPU.
//!
//! ## Substrate model
//!
//! Pair-set membership ("which cats are within range of which other cats")
//! changes only when a cat moves — not every tick. The cache is built
//! incrementally on [`CatMoved`](crate::messages::cat_moved::CatMoved)
//! messages emitted by `emit_cat_moved_messages` (Stage A). The consumer
//! `passive_familiarity` reads the cache and applies the per-tick delta
//! over the cached pair set without re-scanning all pairs every tick.
//!
//! ## Determinism
//!
//! Keys are normalized as `(a, b)` with `a.index() <= b.index()` so each
//! unordered pair appears exactly once. This mirrors `Relationships`'s
//! `normalize_key` ([`src/resources/relationships.rs:69-76`](crate::resources::relationships))
//! so the two maps' iteration orders agree at the pair level. The
//! `BTreeMap` yields entries in key order — process-independent, run-
//! independent, the load-bearing float-determinism contract.
//!
//! Per-pair familiarity updates are `+= delta` against an independent
//! map entry (one entry per unordered pair), so pair iteration order
//! does not affect the final state — each pair receives exactly one
//! increment per tick. The float-non-associativity warning at
//! `relationships.rs:55-63` bites only when **summing many relationships
//! together** (e.g., the coordinator's `social_weight` sum over
//! `all_for(entity)`); single-pair `+=` is order-independent.
//!
//! The stored value is the cached Manhattan distance (`i32`, no float
//! semantics). Future consumers may read it without recomputing.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

/// Normalize an unordered cat pair to `(min, max)` by `Entity::index()`.
/// Mirrors `Relationships`'s `normalize_key` so the two maps' pair-level
/// canonicalization agrees.
pub fn normalize_pair(a: Entity, b: Entity) -> (Entity, Entity) {
    if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    }
}

/// Cat pairs within `passive_familiarity_range` of each other and their
/// last-known Manhattan distance. Built incrementally from `CatMoved`
/// messages by `update_near_pair_cache` (Stage B); read by
/// `passive_familiarity` for the per-tick delta application.
///
/// `last_seen` tracks the live cat set as of the previous tick so the
/// cache update can detect newborn cats (in `live` but not in `last_seen`)
/// and pull them into a re-scan. Without this, newborns would have no
/// pair-entries until their first movement event — they'd silently lose
/// passive familiarity for their first few ticks.
#[derive(Resource, Debug, Default)]
pub struct NearPairCache {
    /// Normalized `(min, max)` pair keys → cached Euclidean distance
    /// (tile-domain f32). Ticket 492 switched from Manhattan.
    pub pairs: BTreeMap<(Entity, Entity), f32>,
    /// Set of live cat entities observed in the previous tick. Used by
    /// `update_near_pair_cache` to detect newborns (entities in the
    /// current `live` set but absent from `last_seen`).
    pub last_seen: BTreeSet<Entity>,
}
