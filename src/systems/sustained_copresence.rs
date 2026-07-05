//! `SustainedCoPresence` cue tracker — accumulates consecutive-tick co-presence
//! durations for cat pairs and emits a `WitnessableEvent::SustainedCoPresence`
//! when a pair's accumulated count crosses the threshold (with a per-pair
//! cooldown). Ticket 279.
//!
//! ## Per-tick discipline
//!
//! This system is per-tick by necessity, not by default. The cue measures
//! continuous in-range duration — an event-driven shape (e.g. firing only
//! on `CatMoved`) would *miss* the steady-state case where two cats sit
//! together without moving for many ticks, which is precisely the signal
//! the cue is shaped to capture. Per CLAUDE.md ECS rules ("default to
//! event-driven; justify per-tick"), the justification is the sustained-
//! accumulation semantics.
//!
//! Cost shape: the per-tick loop iterates `Res<NearPairCache>.pairs` (the
//! event-driven pair set from ticket 431 Stage B). Steady-state pair counts
//! are bounded by the colony size times the average near-pair degree —
//! orders of magnitude smaller than the O(N²) sweep this leverages.
//!
//! ## Determinism
//!
//! `pair_ticks` and `last_emit` are `BTreeMap` keyed by `normalize_pair(a, b)`
//! — the same canonicalization `NearPairCache` and `Relationships` use, so
//! iteration order is process-independent and stable across ticks.
//!
//! ## Emission shape
//!
//! When a pair `(a, b)` crosses the threshold and the per-pair cooldown has
//! elapsed, we emit **both** `WitnessableEvent::SustainedCoPresence { actor: a,
//! target: b, .. }` AND its sibling `{ actor: b, target: a, .. }`. The
//! integrator's per-cat lift is directional (lifts witness's belief about
//! `actor`), so both directions need their own event payload to populate the
//! reciprocal lifts.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::components::physical::{Dead, Position};
use crate::messages::witnessable_event::WitnessableEvent;
use crate::resources::near_pair_cache::{normalize_pair, NearPairCache};
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::TimeState;

/// Per-pair sustained-co-presence accumulator. Counts consecutive ticks each
/// pair appears in `NearPairCache.pairs`; resets when the pair drops out;
/// emits + resets when the count crosses
/// `cfg.sustained_copresence_threshold_ticks` and the per-pair cooldown has
/// elapsed.
///
/// Ticket 485: each `pair_ticks` value carries a `last_touched_tick` so
/// discontinuous co-presence (pair drops and reappears) resets the counter
/// without the prior tick-by-tick `BTreeMap::retain` walk over every entry.
#[derive(Resource, Debug, Default)]
pub struct SustainedCoPresenceTracker {
    /// Normalized `(min, max)` pair keys → (consecutive-tick counter,
    /// last tick this pair was observed in `NearPairCache.pairs`). The
    /// `last_touched_tick` lets the per-tick loop detect discontinuities
    /// (pair dropped out for one or more ticks) without a full-map retain.
    pub pair_ticks: BTreeMap<(Entity, Entity), (u32, u64)>,
    /// Normalized `(min, max)` pair keys → tick of the most recent emission
    /// (for per-pair cooldown).
    pub last_emit: BTreeMap<(Entity, Entity), u64>,
}

/// Per-tick: increment counters for each pair still in the cache, evict
/// stale pairs, and emit when threshold + cooldown both pass.
pub fn track_sustained_copresence(
    mut tracker: ResMut<SustainedCoPresenceTracker>,
    cache: Res<NearPairCache>,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
    cats: Query<&Position, Without<Dead>>,
    mut events: MessageWriter<WitnessableEvent>,
) {
    let tick = time.tick;
    let cfg = &constants.play_cue_emission;
    let threshold = cfg.sustained_copresence_threshold_ticks;
    let cooldown = cfg.sustained_copresence_emit_cooldown_ticks;
    let prev = tick.saturating_sub(1);

    // Ticket 504 — merge-join co-walk. `cache.pairs` and
    // `tracker.pair_ticks` are BTreeMaps over the same
    // `normalize_pair`-canonicalized key, so one two-cursor sweep
    // replaces the pre-504 shape: a per-tick `Vec` collect of every
    // cache key plus one O(log n) `entry` root-descent per pair
    // (19.66% self CPU at the 07-05 post-500 flamegraph — same knife
    // shape 500 removed from `passive_familiarity`). The 485 comment
    // justified the key-Vec as borrow hygiene, but `Res<NearPairCache>`
    // and `ResMut<SustainedCoPresenceTracker>` are disjoint system
    // params — there was never an aliasing problem. Field-destructuring
    // the tracker lets the matched arm mutate `pair_ticks` entries in
    // place while the emit arm reads/writes `last_emit`.
    //
    // The 485 lazy-eviction semantics are unchanged: stale tracker
    // entries (pair no longer cached) are skipped by the cursor (their
    // `last_touched_tick` goes stale exactly as before) and swept by
    // the periodic GC below.
    let tracker = &mut *tracker;
    let pair_ticks = &mut tracker.pair_ticks;
    let last_emit = &mut tracker.last_emit;

    // Cache pairs with no tracker entry yet — inserted after the walk
    // (can't insert into `pair_ticks` mid-`iter_mut`). Their fresh
    // count is 1, so the threshold branch is unreachable for them at
    // any threshold > 1 (canonical config); the shared closure keeps
    // pathological configs correct, at the cost of those emissions
    // sorting after the matched-arm emissions within the same tick
    // (default-config byte stream is unaffected).
    let mut new_pairs: Vec<(Entity, Entity)> = Vec::new();
    // Entries whose endpoint despawned this tick — removed after the
    // walk (mid-iteration removal is what the pre-504 Vec allowed).
    let mut dead_keys: Vec<(Entity, Entity)> = Vec::new();

    // Threshold/cooldown/emit tail, shared by the matched arm and the
    // (rare) fresh-pair path. Returns `false` when the pair's entry
    // must be dropped because an endpoint despawned this tick.
    let fire_check = |key: (Entity, Entity),
                      count_slot: &mut u32,
                      last_emit: &mut BTreeMap<(Entity, Entity), u64>,
                      events: &mut MessageWriter<WitnessableEvent>|
     -> bool {
        let count = *count_slot;
        if count < threshold {
            return true;
        }
        let in_cooldown = last_emit
            .get(&key)
            .copied()
            .map(|last_tick| tick.saturating_sub(last_tick) < cooldown)
            .unwrap_or(false);
        if in_cooldown {
            *count_slot = 0;
            return true;
        }
        // Need both cats' Position to populate the event. If either
        // lookup misses (e.g. the cat despawned this same tick), drop
        // the pair — the next-tick path will not re-touch it and the
        // periodic GC below sweeps it.
        let (a, b) = key;
        let Ok(pos_a) = cats.get(a) else {
            return false;
        };
        if cats.get(b).is_err() {
            return false;
        }
        // Emit symmetrically: each direction carries its own
        // `actor`/`target` so the integrator's per-cat lift fires for
        // both directions of the pair's mutual perception.
        events.write(WitnessableEvent::SustainedCoPresence {
            actor: a,
            target: b,
            ticks_held: count,
            position: *pos_a,
            tick,
        });
        events.write(WitnessableEvent::SustainedCoPresence {
            actor: b,
            target: a,
            ticks_held: count,
            position: *pos_a,
            tick,
        });
        *count_slot = 0;
        last_emit.insert(key, tick);
        true
    };

    let mut cache_keys = cache.pairs.keys().copied().peekable();
    for (&key, entry) in pair_ticks.iter_mut() {
        loop {
            match cache_keys.peek() {
                Some(&k) if k < key => {
                    new_pairs.push(k);
                    cache_keys.next();
                }
                Some(&k) if k == key => {
                    // Matched — increment (or reset on discontinuity:
                    // the pair was not observed on the immediately
                    // preceding tick; `tick == 0` bootstrap stays
                    // well-defined via `saturating_sub`).
                    if entry.1 != prev {
                        entry.0 = 1;
                    } else {
                        entry.0 = entry.0.saturating_add(1);
                    }
                    entry.1 = tick;
                    if !fire_check(key, &mut entry.0, last_emit, &mut events) {
                        dead_keys.push(key);
                    }
                    cache_keys.next();
                    break;
                }
                // Cache exhausted, or this tracker entry is stale
                // (lazy-evicted pair) — move to the next entry.
                _ => break,
            }
        }
    }
    // Cache pairs beyond the last tracker entry.
    for k in cache_keys {
        new_pairs.push(k);
    }
    for key in dead_keys {
        pair_ticks.remove(&key);
    }
    for key in new_pairs {
        // Fresh entry: first observed tick → count 1, exactly what the
        // pre-504 `or_insert((0, tick))` + discontinuity-reset produced.
        let mut slot = (1u32, tick);
        if fire_check(key, &mut slot.0, last_emit, &mut events) {
            pair_ticks.insert(key, slot);
        }
    }

    // Periodic GC. The lazy-eviction shape leaves stale entries in
    // `pair_ticks` and `last_emit` after their pair drops out of the cache.
    // For ~28 pair-pairs at peak population the memory cost is negligible,
    // but bound it anyway by running a full retain every `gc_period` ticks
    // (5 cooldown windows = ~1000 ticks at default tuning). Skip on
    // `tick == 0` so the cold-start path doesn't pay it.
    let gc_period = cooldown.saturating_mul(5).max(1);
    if tick != 0 && tick.is_multiple_of(gc_period) {
        let cutoff = tick.saturating_sub(cooldown);
        tracker
            .pair_ticks
            .retain(|key, _| cache.pairs.contains_key(key));
        tracker
            .last_emit
            .retain(|key, &mut emitted_at| cache.pairs.contains_key(key) || emitted_at > cutoff);
    }

    // Ensure normalize_pair is used (defensive — the cache uses it already,
    // but this guards against any future caller that builds keys outside
    // the cache).
    debug_assert!(tracker
        .pair_ticks
        .keys()
        .all(|&(a, b)| normalize_pair(a, b) == (a, b)));

    // Ticket 485 debug-only invariant (431 Stage B precedent). After this
    // system runs, every pair currently in `NearPairCache.pairs` must have
    // been touched this tick — either via increment in the main loop or
    // via the cooldown/emit-reset branches (which also stamp last_touched).
    // The only legitimate gap is a pair the loop removed via the
    // `cats.get(_).is_err()` arm (one of the endpoints despawned this
    // same tick) — those are absent from `pair_ticks` rather than stale.
    // Catches drift if any branch fails to stamp `last_touched_tick`.
    #[cfg(debug_assertions)]
    for key in cache.pairs.keys() {
        if let Some(&(_, last)) = tracker.pair_ticks.get(key) {
            assert_eq!(
                last, tick,
                "copresence drift: cache pair {:?} present but pair_ticks.last_touched = {} (tick = {})",
                key, last, tick
            );
        }
    }
}
