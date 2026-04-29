//! `RecentTargetFailures` — per-cat memory of recently-failed
//! `(action, target)` pairs (ticket 073, sub-epic 071).
//!
//! Closes audit gap #1 from `tickets/071-planning-substrate-hardening.md`:
//! the substrate's per-plan `GoapPlan::failed_actions` set is destroyed
//! when the plan is abandoned, so the cat re-picks the same blocked
//! target on the very next plan. The seed-42 stuck-loop pattern (Nettle
//! 66× `TravelTo(SocialTarget)`, Mocha 109× `HarvestCarcass`, Lark 91×
//! `EngageThreat`) recurs because nothing in the substrate persists
//! "this target failed me" across plan abandonment.
//!
//! This component is the *memory*. Its first reader is the
//! `target_recent_failure` Consideration registered on the six target-
//! taking DSEs (audit gap #2) — that consideration multiplies a recently-
//! failed candidate's product score down toward 0.1 and recovers
//! linearly over `target_failure_cooldown_ticks`.
//!
//! ## Shape
//!
//! - `failures: HashMap<(GoapActionKind, Entity), u64>` — value is the
//!   tick at which the most recent failure was recorded. Re-writing an
//!   existing key on a new failure resets the cooldown clock.
//! - Inserted *lazily* on first failure (the spawn bundle doesn't pay
//!   for it on cats that never fail a target). The
//!   `prune_recent_target_failures` maintenance system in chain 2a
//!   bounds map size by expiring entries older than the cooldown
//!   window.
//!
//! ## Read site
//!
//! `plan_substrate::target_recent_failure_age_normalized` — pure
//! function over `(now, failed_tick, cooldown_ticks)` that yields a
//! `[0, 1]` signal: 0.0 = just failed (full cooldown), 1.0 = no
//! recorded failure or fully expired (no cooldown).
//!
//! ## Write sites
//!
//! `plan_substrate::lifecycle::record_step_failure` and
//! `plan_substrate::lifecycle::abandon_plan` both write when they're
//! called with a known failed `(action, target)` pair — the substrate
//! API is the single owner of this component's mutations.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::ai::planner::GoapActionKind;

/// Per-cat memory of recently-failed `(action, target)` pairs. See the
/// module docs for placement, lifecycle, and contract notes.
///
/// `failures.get(&(action, target))` returns the **tick** at which the
/// pair last failed. Cooldown age is `now - failed_tick`. Higher-level
/// code never reads the raw map; the `target_recent_failure_age_normalized`
/// sensor in `plan_substrate` is the only sanctioned read.
#[derive(Component, Debug, Clone, Default)]
pub struct RecentTargetFailures {
    pub failures: HashMap<(GoapActionKind, Entity), u64>,
}

impl RecentTargetFailures {
    /// Record (or refresh) a failure. Re-writing an existing key on a
    /// new failure resets the cooldown clock — a cat that fails the
    /// same target twice in a row gets a fresh full cooldown rather
    /// than an averaged half-cooldown.
    pub fn record(&mut self, action: GoapActionKind, target: Entity, tick: u64) {
        self.failures.insert((action, target), tick);
    }

    /// Lookup the tick of the most recent `(action, target)` failure,
    /// if any. `None` means no recorded failure for the pair.
    pub fn last_failure_tick(&self, action: GoapActionKind, target: Entity) -> Option<u64> {
        self.failures.get(&(action, target)).copied()
    }

    /// Drop entries older than `cooldown_ticks`. Returns the number of
    /// entries removed (useful for telemetry / tests). Bounds per-cat
    /// map size; called from `prune_recent_target_failures` in chain
    /// 2a's decay batch.
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

    fn entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).unwrap()
    }

    #[test]
    fn record_inserts_pair_with_tick() {
        let mut r = RecentTargetFailures::default();
        let target = entity(10);
        r.record(GoapActionKind::SocializeWith, target, 1234);
        assert_eq!(
            r.last_failure_tick(GoapActionKind::SocializeWith, target),
            Some(1234)
        );
    }

    #[test]
    fn record_refreshes_existing_key() {
        let mut r = RecentTargetFailures::default();
        let target = entity(10);
        r.record(GoapActionKind::SocializeWith, target, 100);
        r.record(GoapActionKind::SocializeWith, target, 500);
        // Latest write wins — cooldown clock resets to the new failure.
        assert_eq!(
            r.last_failure_tick(GoapActionKind::SocializeWith, target),
            Some(500)
        );
        assert_eq!(r.len(), 1, "refreshed key should not duplicate");
    }

    #[test]
    fn lookup_distinguishes_action_and_target() {
        let mut r = RecentTargetFailures::default();
        let a = entity(10);
        let b = entity(11);
        r.record(GoapActionKind::SocializeWith, a, 100);
        r.record(GoapActionKind::GroomOther, a, 200);
        r.record(GoapActionKind::SocializeWith, b, 300);
        assert_eq!(
            r.last_failure_tick(GoapActionKind::SocializeWith, a),
            Some(100)
        );
        assert_eq!(
            r.last_failure_tick(GoapActionKind::GroomOther, a),
            Some(200)
        );
        assert_eq!(
            r.last_failure_tick(GoapActionKind::SocializeWith, b),
            Some(300)
        );
        assert_eq!(r.last_failure_tick(GoapActionKind::GroomOther, b), None);
    }

    #[test]
    fn prune_drops_entries_older_than_cooldown() {
        let mut r = RecentTargetFailures::default();
        let a = entity(10);
        let b = entity(11);
        r.record(GoapActionKind::SocializeWith, a, 100); // age 9000 at now=9100
        r.record(GoapActionKind::SocializeWith, b, 5000); // age 4100
        let removed = r.prune_expired(9100, 8000);
        assert_eq!(removed, 1, "the older-than-cooldown entry is dropped");
        assert_eq!(r.last_failure_tick(GoapActionKind::SocializeWith, a), None);
        assert_eq!(
            r.last_failure_tick(GoapActionKind::SocializeWith, b),
            Some(5000)
        );
    }

    #[test]
    fn prune_keeps_entries_at_exactly_cooldown_ticks() {
        // age == cooldown means "expired"; age < cooldown means "kept".
        // The boundary lives on the kept side of the cliff: an entry
        // recorded `cooldown_ticks - 1` ticks ago survives.
        let mut r = RecentTargetFailures::default();
        let target = entity(10);
        r.record(GoapActionKind::SocializeWith, target, 100);
        // age = 7999 at now=8099 → kept
        let removed = r.prune_expired(8099, 8000);
        assert_eq!(removed, 0);
        // age = 8000 at now=8100 → expired
        let removed = r.prune_expired(8100, 8000);
        assert_eq!(removed, 1);
    }

    #[test]
    fn prune_handles_now_less_than_failed_tick_defensively() {
        // Defensive: if a system clock somehow rewinds (test fixture
        // edge cases, save reload), saturating_sub keeps age at 0,
        // entries are kept (no spurious expiry).
        let mut r = RecentTargetFailures::default();
        let target = entity(10);
        r.record(GoapActionKind::SocializeWith, target, 5000);
        let removed = r.prune_expired(100, 8000);
        assert_eq!(removed, 0);
        assert_eq!(
            r.last_failure_tick(GoapActionKind::SocializeWith, target),
            Some(5000)
        );
    }
}
