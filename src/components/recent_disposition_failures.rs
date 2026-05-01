//! `RecentDispositionFailures` — per-cat memory of recently-failed
//! dispositions (ticket 123, sub-epic 071).
//!
//! Closes the IAUS↔planner gap that produces same-tick
//! `PlanningFailed/no_plan_found` retry storms: when `make_plan`
//! returns `None` for a chosen disposition (e.g., `Crafting` with no
//! kitchen, `Foraging` with no nearby foraging tile, `Hunting` with
//! no prey in range), the cat's IAUS scoring is unchanged by the
//! failure, so the same disposition typically wins again the next
//! tick and the same `make_plan → None` collapse repeats. In the
//! seed-42 cold-start window for ticket 121, this produced 3059
//! wasted planning rounds in 1500 ticks.
//!
//! This component is the *memory*. Its first reader is the
//! `disposition_recent_failure` Consideration registered on the six
//! failure-prone cat-action DSEs (Hunting, Foraging, Crafting,
//! Caretaking, Building, Mating) — that consideration multiplies the
//! disposition's IAUS score down toward 0.1 and recovers linearly
//! over `disposition_failure_cooldown_ticks`. Resting and Guarding
//! are exempt because their step graphs don't share the
//! `_ => trips_done >= target_trips` failure family.
//!
//! ## Shape
//!
//! - `failures: HashMap<DispositionKind, u64>` — value is the tick at
//!   which the most recent failure was recorded. Re-writing an
//!   existing key on a new failure resets the cooldown clock.
//!   Mirrors `RecentTargetFailures` exactly: tick-only, no count
//!   tracking. The simpler shape is sufficient because the cooldown
//!   curve already floors at 0.1 on first failure, and a refreshed
//!   tick on repeat failure resets the cooldown clock back to
//!   maximum penalty.
//! - Inserted *lazily* on first failure (the spawn bundle doesn't
//!   pay for it on cats that never fail a disposition). The
//!   `prune_recent_disposition_failures` maintenance system in chain
//!   2a bounds map size by expiring entries older than the cooldown
//!   window.
//!
//! ## Read site
//!
//! `plan_substrate::disposition_recent_failure_age_normalized` —
//! pure function over `(now, failed_tick, cooldown_ticks)` that
//! yields a `[0, 1]` signal: 0.0 = just failed (full cooldown), 1.0
//! = no recorded failure or fully expired (no cooldown).
//!
//! ## Write site
//!
//! `evaluate_and_plan` (`src/systems/goap.rs`) writes here on the
//! `make_plan → None` branch alongside the `PlanningFailed` event
//! emission. The substrate API doesn't own this write because the
//! per-tick scoring loop already holds the cat's components and
//! `Commands` borrow; routing the write through a substrate helper
//! would require flowing per-cat state across system boundaries
//! without local benefit.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::components::DispositionKind;

/// Per-cat memory of recently-failed dispositions. See the module
/// docs for placement, lifecycle, and contract notes.
///
/// `failures.get(&kind)` returns the **tick** at which the disposition
/// last hit `make_plan → None`. Cooldown age is `now - failed_tick`.
/// Higher-level code never reads the raw map; the
/// `disposition_recent_failure_age_normalized` sensor in
/// `plan_substrate` is the only sanctioned read.
#[derive(Component, Debug, Clone, Default)]
pub struct RecentDispositionFailures {
    pub failures: HashMap<DispositionKind, u64>,
}

impl RecentDispositionFailures {
    /// Record (or refresh) a failure. Re-writing an existing key on a
    /// new failure resets the cooldown clock — a cat that fails the
    /// same disposition twice in a row gets a fresh full cooldown
    /// rather than an averaged half-cooldown.
    pub fn record(&mut self, kind: DispositionKind, tick: u64) {
        self.failures.insert(kind, tick);
    }

    /// Lookup the tick of the most recent failure for `kind`, if any.
    /// `None` means no recorded failure for the disposition.
    pub fn last_failure_tick(&self, kind: DispositionKind) -> Option<u64> {
        self.failures.get(&kind).copied()
    }

    /// Drop entries older than `cooldown_ticks`. Returns the number
    /// of entries removed (useful for telemetry / tests). Bounds
    /// per-cat map size; called from
    /// `prune_recent_disposition_failures` in chain 2a's decay batch.
    pub fn prune_expired(&mut self, now: u64, cooldown_ticks: u64) -> usize {
        let before = self.failures.len();
        self.failures
            .retain(|_, &mut failed_tick| now.saturating_sub(failed_tick) < cooldown_ticks);
        before - self.failures.len()
    }

    /// Number of stored entries — exposed for tests and diagnostics.
    pub fn len(&self) -> usize {
        self.failures.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_inserts_kind_with_tick() {
        let mut r = RecentDispositionFailures::default();
        r.record(DispositionKind::Crafting, 1234);
        assert_eq!(r.last_failure_tick(DispositionKind::Crafting), Some(1234));
    }

    #[test]
    fn record_refreshes_existing_key() {
        let mut r = RecentDispositionFailures::default();
        r.record(DispositionKind::Foraging, 100);
        r.record(DispositionKind::Foraging, 500);
        // Latest write wins — cooldown clock resets to the new failure.
        assert_eq!(r.last_failure_tick(DispositionKind::Foraging), Some(500));
        assert_eq!(r.len(), 1, "refreshed key should not duplicate");
    }

    #[test]
    fn lookup_distinguishes_dispositions() {
        let mut r = RecentDispositionFailures::default();
        r.record(DispositionKind::Hunting, 100);
        r.record(DispositionKind::Foraging, 200);
        r.record(DispositionKind::Crafting, 300);
        assert_eq!(r.last_failure_tick(DispositionKind::Hunting), Some(100));
        assert_eq!(r.last_failure_tick(DispositionKind::Foraging), Some(200));
        assert_eq!(r.last_failure_tick(DispositionKind::Crafting), Some(300));
        assert_eq!(r.last_failure_tick(DispositionKind::Building), None);
    }

    #[test]
    fn prune_drops_entries_older_than_cooldown() {
        let mut r = RecentDispositionFailures::default();
        r.record(DispositionKind::Hunting, 100); // age 4000 at now=4100
        r.record(DispositionKind::Foraging, 3000); // age 1100
        let removed = r.prune_expired(4100, 4000);
        assert_eq!(removed, 1, "the older-than-cooldown entry is dropped");
        assert_eq!(r.last_failure_tick(DispositionKind::Hunting), None);
        assert_eq!(r.last_failure_tick(DispositionKind::Foraging), Some(3000));
    }

    #[test]
    fn prune_keeps_entries_at_exactly_cooldown_minus_one() {
        // age == cooldown means "expired"; age < cooldown means
        // "kept". The boundary lives on the kept side of the cliff:
        // an entry recorded `cooldown_ticks - 1` ticks ago survives.
        let mut r = RecentDispositionFailures::default();
        r.record(DispositionKind::Crafting, 100);
        // age = 3999 at now=4099 → kept
        let removed = r.prune_expired(4099, 4000);
        assert_eq!(removed, 0);
        // age = 4000 at now=4100 → expired
        let removed = r.prune_expired(4100, 4000);
        assert_eq!(removed, 1);
    }

    #[test]
    fn prune_handles_now_less_than_failed_tick_defensively() {
        // Defensive: if a system clock somehow rewinds (test fixture
        // edge cases, save reload), saturating_sub keeps age at 0,
        // entries are kept (no spurious expiry).
        let mut r = RecentDispositionFailures::default();
        r.record(DispositionKind::Hunting, 5000);
        let removed = r.prune_expired(100, 4000);
        assert_eq!(removed, 0);
        assert_eq!(r.last_failure_tick(DispositionKind::Hunting), Some(5000));
    }
}
