//! `Reserved` — resource-reservation marker (ticket 080).
//!
//! Attached to an entity (carcass, herb tile, prey, build site, mate)
//! when a cat commits to it as a plan target. Other cats see the
//! candidate as ineligible during the reservation window via
//! `EligibilityFilter::require_unreserved`; the owner cat continues to
//! score the target normally.
//!
//! Lifecycle:
//! - **Write**: `plan_substrate::target::reserve_target(commands, target,
//!   owner, tick, ttl)` after a target picker resolves a winning target.
//! - **Release**: `plan_substrate::target::release_target(commands,
//!   target)` on terminal failure of a Harvest/Build/Mate step or when
//!   `plan_substrate::lifecycle::abandon_plan` returns reservations to
//!   release.
//! - **Expire**: `systems::reservation::expire_reservations` runs in
//!   chain 2a's decay batch and removes `Reserved` whose `expires_tick`
//!   is in the past. Bounds the world-size of the marker.
//!
//! `Default` is intentionally not derived — an empty `Reserved` is
//! meaningless and would silently gate every cat off the entity.

use bevy_ecs::prelude::*;

/// Per-entity reservation. `owner` is the cat that committed to the
/// target; `expires_tick` is the absolute simulation tick after which
/// the maintenance system removes the marker.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reserved {
    pub owner: Entity,
    pub expires_tick: u64,
}

impl Reserved {
    /// Build a fresh reservation expiring at `tick + ttl_ticks`.
    pub fn new(owner: Entity, tick: u64, ttl_ticks: u64) -> Self {
        Self {
            owner,
            expires_tick: tick.saturating_add(ttl_ticks),
        }
    }

    /// True iff `tick` is at or past the expiry boundary.
    pub fn is_expired(&self, tick: u64) -> bool {
        tick >= self.expires_tick
    }

    /// True iff `cat` is the owner of this reservation.
    pub fn is_owned_by(&self, cat: Entity) -> bool {
        self.owner == cat
    }
}
