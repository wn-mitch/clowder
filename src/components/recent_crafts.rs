//! `CatRecentCrafts` — per-cat fixed-size ring buffer recording the last
//! N crafted recipes and the tick each was made (ticket 463).
//!
//! ## Why
//!
//! `CraftItemAspiration` (also ticket 463) scores each satisfiable
//! recipe per cat per tick by three axes; one of the three is an
//! **anti-monotony** term `−W_recent / (1 + ticks_since(recipe.id))`
//! that pressures the aspiration to diversify across the colony's
//! warrior's-kit set over the soak window. Without anti-monotony, the
//! threat-cue + skill-affinity scores degenerate to "pick the same
//! recipe every tick" — the variation gate (≥3 distinct kit items per
//! 900-tick soak) is the load-bearing diversity invariant.
//!
//! ## Shape
//!
//! Fixed 8-slot ring buffer of `(Option<RecipeId>, u64)`. 8 slots gives
//! enough horizon to span the 16 currently-craftable recipes' typical
//! lex-cycle without burning memory; the ring index lives in `head: u8`
//! and wraps at 8. Empty slots carry `None` so a freshly-spawned cat
//! reads `ticks_since(_) == None` for every recipe (the aspiration's
//! scoring treats `None` as "no recency penalty — free first attempt").
//!
//! ## Lifecycle
//!
//! - Mounted on every `Cat` at spawn with `Default::default()` (eight
//!   `None` slots).
//! - Written by the craft-success caller in
//!   `crate::systems::goap::resolve_goap_plans` whenever a
//!   `GoapActionKind::CraftAtWorkshop` / `CraftAtTanningFrame` step
//!   returns a witnessed `Advance` — the witness payload carries the
//!   `RecipeId` of the actually-crafted recipe, which the caller
//!   threads into `record(recipe_id, tick)` on the cat's
//!   `CatRecentCrafts`. Drain pattern mirrors `handoff_pending` at
//!   goap.rs:7708-7713.
//! - Read by the aspiration picker's recipe-scoring loop via
//!   `ticks_since(recipe_id, now) -> Option<u64>`.

use bevy_ecs::prelude::*;

use crate::components::recipe::RecipeId;

/// Capacity of the per-cat ring buffer. Sized to span the current 16-
/// recipe registry's typical lex-cycle without dominating cat-Component
/// memory.
const RECENT_CRAFTS_CAPACITY: usize = 8;

/// Per-cat ring buffer recording the last `RECENT_CRAFTS_CAPACITY`
/// crafted recipes plus the tick each was made. See module docs for
/// lifecycle and contract notes.
#[derive(Component, Debug, Clone, serde::Serialize)]
pub struct CatRecentCrafts {
    entries: [(Option<RecipeId>, u64); RECENT_CRAFTS_CAPACITY],
    head: u8,
}

impl Default for CatRecentCrafts {
    fn default() -> Self {
        Self {
            entries: [(None, 0); RECENT_CRAFTS_CAPACITY],
            head: 0,
        }
    }
}

impl CatRecentCrafts {
    /// Append a craft event at the current `head` and advance the
    /// cursor. Overwrites the oldest slot once the buffer is full.
    pub fn record(&mut self, recipe_id: RecipeId, tick: u64) {
        self.entries[self.head as usize] = (Some(recipe_id), tick);
        self.head = ((self.head as usize + 1) % RECENT_CRAFTS_CAPACITY) as u8;
    }

    /// Return `Some(now - failed_tick)` for the most recent entry
    /// matching `recipe_id`, or `None` when the recipe never landed in
    /// the buffer (or has already been evicted by newer crafts).
    /// Returns `None` if `tick < now` would underflow (a planner-state
    /// invariant violation; the caller's scoring fn treats `None` the
    /// same as "no recent penalty").
    pub fn ticks_since(&self, recipe_id: RecipeId, now: u64) -> Option<u64> {
        // Walk the buffer; pick the most-recent entry by `tick` value
        // (the ring's head/tail ordering would also work, but iterating
        // by `tick` makes the read robust to head-wrap semantics).
        let mut latest: Option<u64> = None;
        for &(rid, tick) in &self.entries {
            if rid == Some(recipe_id) {
                match latest {
                    Some(l) if tick > l => latest = Some(tick),
                    None => latest = Some(tick),
                    _ => {}
                }
            }
        }
        latest.and_then(|t| now.checked_sub(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_reports_no_recent() {
        let buf = CatRecentCrafts::default();
        assert_eq!(buf.ticks_since(RecipeId("x"), 100), None);
    }

    #[test]
    fn record_then_ticks_since_returns_age() {
        let mut buf = CatRecentCrafts::default();
        buf.record(RecipeId("bone_tip_spear"), 100);
        assert_eq!(buf.ticks_since(RecipeId("bone_tip_spear"), 250), Some(150));
        // Unmatched recipe id reads as None even though buffer is non-empty.
        assert_eq!(buf.ticks_since(RecipeId("sling"), 250), None);
    }

    #[test]
    fn ninth_insert_evicts_oldest() {
        let mut buf = CatRecentCrafts::default();
        // Fill the buffer with 8 unique recipes at ticks 1..=8.
        let ids = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7"];
        for (i, id) in ids.iter().enumerate() {
            buf.record(RecipeId(id), (i + 1) as u64);
        }
        // Insert a 9th — evicts r0 (oldest).
        buf.record(RecipeId("r8"), 9);
        assert!(buf.ticks_since(RecipeId("r0"), 100).is_none());
        assert_eq!(buf.ticks_since(RecipeId("r1"), 100), Some(98));
        assert_eq!(buf.ticks_since(RecipeId("r8"), 100), Some(91));
    }

    #[test]
    fn repeated_record_keeps_latest_tick() {
        let mut buf = CatRecentCrafts::default();
        buf.record(RecipeId("r"), 10);
        buf.record(RecipeId("r"), 50);
        // Most recent wins.
        assert_eq!(buf.ticks_since(RecipeId("r"), 100), Some(50));
    }
}
