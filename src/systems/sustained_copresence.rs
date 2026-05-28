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

    // Walk the cache in BTreeMap key order; for each live pair, increment
    // (or reset, on discontinuity) its counter, check threshold + cooldown,
    // emit + reset on fire.
    //
    // Ticket 485: collecting the keys before the entry loop lets us hold a
    // `&mut tracker` exclusively inside the loop without aliasing
    // `cache.pairs` (the prior shape pre-collected the same Vec). The
    // `last_touched_tick` carried in each entry handles eviction lazily —
    // a pair that drops out has its entry stay in the map but with a stale
    // `last_touched_tick`; if the pair returns, the discontinuity check
    // resets the count.
    let pair_keys: Vec<(Entity, Entity)> = cache.pairs.keys().copied().collect();
    for key in pair_keys {
        // Increment the counter in a tight scope so the mutable borrow on
        // `tracker.pair_ticks` ends before we look at `tracker.last_emit`.
        let count = {
            let entry = tracker.pair_ticks.entry(key).or_insert((0, tick));
            // Discontinuity check: if we did not observe this pair on the
            // immediately preceding tick, the run resets. `tick == 0` is
            // the bootstrap case (saturating_sub keeps it well-defined).
            let prev = tick.saturating_sub(1);
            if entry.1 != prev {
                entry.0 = 1;
            } else {
                entry.0 = entry.0.saturating_add(1);
            }
            entry.1 = tick;
            entry.0
        };

        if count < threshold {
            continue;
        }

        // Cooldown check — skip if we emitted for this pair recently.
        let in_cooldown = tracker
            .last_emit
            .get(&key)
            .copied()
            .map(|last_tick| tick.saturating_sub(last_tick) < cooldown)
            .unwrap_or(false);
        if in_cooldown {
            if let Some(c) = tracker.pair_ticks.get_mut(&key) {
                c.0 = 0;
            }
            continue;
        }

        // Need both cats' Position to populate the event. If either lookup
        // misses (e.g. the cat despawned this same tick), drop the pair —
        // the next-tick path will not re-touch it and the periodic GC below
        // sweeps it.
        let (a, b) = key;
        let Ok(pos_a) = cats.get(a) else {
            tracker.pair_ticks.remove(&key);
            continue;
        };
        let Ok(_pos_b) = cats.get(b) else {
            tracker.pair_ticks.remove(&key);
            continue;
        };
        let ticks_held = count;

        // Emit symmetrically: each direction carries its own `actor`/`target`
        // so the integrator's per-cat lift fires for both directions of the
        // pair's mutual perception.
        events.write(WitnessableEvent::SustainedCoPresence {
            actor: a,
            target: b,
            ticks_held,
            position: *pos_a,
            tick,
        });
        events.write(WitnessableEvent::SustainedCoPresence {
            actor: b,
            target: a,
            ticks_held,
            position: *pos_a,
            tick,
        });

        // Reset the count and stamp the cooldown.
        if let Some(c) = tracker.pair_ticks.get_mut(&key) {
            c.0 = 0;
        }
        tracker.last_emit.insert(key, tick);
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
