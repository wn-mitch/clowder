//! Target-handling operations: validity, carryover across step
//! boundaries, alive-eligibility filter, resource reservation. 072
//! stubbed `validate_target` / `require_alive_filter`; 074 fleshes out
//! `EligibilityFilter::require_alive` + step-resolver `validate_target`
//! (dead targets rejected at scoring time AND at step entry). Ticket
//! 080 adds the resource-reservation API (`reserve_target` /
//! `release_target` / `require_unreserved_filter`).

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::ai::dse::EligibilityFilter;
use crate::components::goap_plan::StepExecutionState;
use crate::components::reserved::Reserved;
use crate::components::markers::{Banished, Incapacitated};
use crate::components::physical::Dead;
use crate::components::RecentTargetFailures;

// ---------------------------------------------------------------------------
// Target validity (074)
// ---------------------------------------------------------------------------

/// Why a target entity is invalid. The four variants mirror the
/// canonical "partner_invalid" predicate used elsewhere in the
/// substrate (`ai::pairing::evaluate_drop`) — keeping the cross-
/// site vocabulary consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetInvalidReason {
    /// Entity has been despawned since the plan committed to it.
    /// Detected when the validity query fails to resolve the entity
    /// (`Query::get(_) == Err(_)`).
    Despawned,
    /// Entity carries the `Dead` component. Cats remain in-world for a
    /// grace period after death (narrative reactions); during that
    /// window they are still queriable but no longer valid targets.
    Dead,
    /// Entity carries the `Banished` faction-overlay marker.
    Banished,
    /// Entity carries the `Incapacitated` state marker (severe
    /// unhealed injury — downed and unable to act).
    Incapacitated,
}

/// Trait abstracting the validity check so the runtime SystemParam
/// path and the test path share the same `validate_target` /
/// `carry_target_forward` entry points. The runtime impl wraps a
/// `Query<(Has<Dead>, Has<Banished>, Has<Incapacitated>)>`; tests can
/// construct an `InMemoryValidity` from a known-invalid set without
/// spinning up a `World`.
pub trait TargetValidity {
    fn check(&self, target: Entity) -> Result<(), TargetInvalidReason>;

    fn is_alive(&self, target: Entity) -> bool {
        self.check(target).is_ok()
    }
}

/// SystemParam bundling the queries needed to check target validity.
/// Read-only; safe to share across systems. Same shape as the
/// `Query<(Has<Dead>, Has<Banished>, Has<Incapacitated>)>` used in
/// `ai::pairing::evaluate_drop` — keeps the validity surface unified.
///
/// 072 shipped a unit struct stub; 074 promotes it to a SystemParam.
/// Callers that already hold the Bevy 16-param budget bundle this in
/// alongside their other queries via `#[derive(SystemParam)]`.
#[derive(SystemParam)]
pub struct TargetValidityQuery<'w, 's> {
    pub query: Query<'w, 's, (Has<Dead>, Has<Banished>, Has<Incapacitated>)>,
}

impl<'w, 's> TargetValidity for TargetValidityQuery<'w, 's> {
    fn check(&self, target: Entity) -> Result<(), TargetInvalidReason> {
        match self.query.get(target) {
            // Despawned — query lookup fails entirely.
            Err(_) => Err(TargetInvalidReason::Despawned),
            Ok((dead, _, _)) if dead => Err(TargetInvalidReason::Dead),
            Ok((_, banished, _)) if banished => Err(TargetInvalidReason::Banished),
            Ok((_, _, incapacitated)) if incapacitated => Err(TargetInvalidReason::Incapacitated),
            Ok(_) => Ok(()),
        }
    }
}

/// In-memory validity predicate for unit tests. Stores the explicit
/// invalidity for known entities; defaults to `Ok(())` for unknown
/// entities. Tests construct this directly without a `World` round-
/// trip.
#[derive(Default)]
pub struct InMemoryValidity {
    pub invalid: std::collections::HashMap<Entity, TargetInvalidReason>,
    /// When `true`, entities not present in `invalid` are reported
    /// `Despawned` — useful for testing the despawn branch.
    pub absent_means_despawned: bool,
}

impl InMemoryValidity {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark(&mut self, e: Entity, reason: TargetInvalidReason) {
        self.invalid.insert(e, reason);
    }
}

impl TargetValidity for InMemoryValidity {
    fn check(&self, target: Entity) -> Result<(), TargetInvalidReason> {
        match self.invalid.get(&target) {
            Some(reason) => Err(*reason),
            None if self.absent_means_despawned => Err(TargetInvalidReason::Despawned),
            None => Ok(()),
        }
    }
}

/// Validate that `target` is still a usable entity for the calling
/// step. Returns `Ok(())` for an alive, non-banished, non-incapacitated
/// entity that still resides in the World. Maps each invalid flavor to
/// a specific [`TargetInvalidReason`] so callers can branch on the
/// cause (e.g., the existing `PlanFailureReason::TargetDespawned` path
/// records the failure category for replanning).
///
/// Step resolvers in `src/steps/disposition/*.rs` and
/// `src/steps/building/*.rs` reach this through their dispatchers in
/// `goap.rs` — the runtime guard that catches mid-step despawn that
/// the IAUS-time `EligibilityFilter::require_alive` couldn't have
/// known about. Belt-and-suspenders: the same predicate runs at both
/// the scoring layer and the execution layer.
pub fn validate_target<V: TargetValidity + ?Sized>(
    target: Entity,
    validity: &V,
) -> Result<(), TargetInvalidReason> {
    validity.check(target)
}

// ---------------------------------------------------------------------------
// carry_target_forward
// ---------------------------------------------------------------------------

/// Carry a step's `target_entity` forward from the prior step when the
/// current step's `target_entity` is `None`. Lifted from
/// `goap.rs:2817–2820`'s `EngagePrey` carryover; 074 wraps the copy
/// in a [`validate_target`] check so dead/banished/incapacitated
/// candidates do **not** propagate across step boundaries.
///
/// **Real-world effect** — when `step_state[step_idx].target_entity`
/// is `None` and `step_idx > 0`, validates the prior step's target and
/// (on success) copies it into the current step. If the prior target
/// is invalid (Dead/Banished/Incapacitated/despawned), this function
/// records the failure into `recent` (when 073's
/// `RecentTargetFailures` is wired) and returns `None`, surfacing the
/// stale target to the caller's existing `PlanStepFailed` path with
/// reason `TargetDespawned`.
///
/// Returns the resulting target entity (or `None` if neither slot
/// held one, or if validation rejected the carryover).
pub fn carry_target_forward<V: TargetValidity + ?Sized>(
    step_state: &mut [StepExecutionState],
    step_idx: usize,
    validity: &V,
    recent: Option<&mut RecentTargetFailures>,
) -> Option<Entity> {
    if step_state[step_idx].target_entity.is_none() && step_idx > 0 {
        if let Some(prior) = step_state[step_idx - 1].target_entity {
            // 074 — gate the copy on validity. A despawned/dead/banished
            // prior target must NOT propagate; the caller's existing
            // `PlanStepFailed` path picks up the `None` and fails the
            // step with reason `TargetDespawned`.
            match validity.check(prior) {
                Ok(()) => {
                    step_state[step_idx].target_entity = Some(prior);
                }
                Err(_) => {
                    // 073's `RecentTargetFailures` will accept the dead
                    // target into the cooldown map when wired. Today
                    // (073 not yet landed in this worktree) the slot is
                    // reserved — we don't write because the component
                    // model isn't yet committed by 073's parallel work;
                    // the `None` return alone is enough to trigger
                    // replan via the caller's failure path.
                    let _ = recent;
                }
            }
        }
    }
    step_state[step_idx].target_entity
}

// ---------------------------------------------------------------------------
// require_alive_filter (074 — IAUS engine extension)
// ---------------------------------------------------------------------------

/// Build an [`EligibilityFilter`] that requires the candidate target
/// to be alive (not Dead / Banished / Incapacitated / despawned).
/// Consumed by the six target-DSE factories via
/// `.eligibility(plan_substrate::require_alive_filter())`.
///
/// The flag is a structural gate distinct from the §4 marker
/// mechanism — the validity facts already live in the per-cat snapshot
/// the resolvers read, so this avoids a parallel marker table and
/// keeps the `EligibilityFilter::require_*` builder convention
/// readable at registration sites.
pub fn require_alive_filter() -> EligibilityFilter {
    EligibilityFilter::new().require_alive()
}

// ---------------------------------------------------------------------------
// Resource reservation (ticket 080)
// ---------------------------------------------------------------------------

/// Build an `EligibilityFilter` whose `require_unreserved` flag tells
/// `evaluate_target_taking` to gate candidates whose `Reserved.owner`
/// is some other cat during the reservation window. Cats that hold the
/// reservation continue to score the candidate normally; non-owners
/// see 0.0.
///
/// Wired via `TargetTakingDse::with_eligibility(...)` (or set on the
/// `eligibility` field at construction). The reservation snapshot the
/// resolver consults to populate `is_reserved_by_other` is the
/// caller's responsibility — the substrate ships the filter shape and
/// the per-candidate predicate; the resolver builds the (cat, target)
/// gate from a frame-local `Reserved` query.
pub fn require_unreserved_filter() -> EligibilityFilter {
    EligibilityFilter::new().require_unreserved()
}

/// Combined alive + unreserved gate. Most target DSEs in tickets 074
/// + 080 want both: only score live, unclaimed candidates. Builders
/// chain `eligibility: require_alive_and_unreserved_filter()` rather
/// than wiring two separate filters.
pub fn require_alive_and_unreserved_filter() -> EligibilityFilter {
    EligibilityFilter::new()
        .require_alive()
        .require_unreserved()
}

/// Write a `Reserved { owner, expires_tick = tick + ttl_ticks }`
/// component to `target`. Idempotent: a fresh `Reserved` overwrites any
/// prior reservation on the same entity (Bevy `Commands::insert`
/// semantics). Callers responsible for invoking after a target picker
/// resolves a winning target.
///
/// **Real-world effect** — schedules an ECS write of `Reserved` on
/// `target`. The write is deferred to the next `Commands` flush per
/// Bevy's normal command-buffer semantics; downstream readers in the
/// same tick will see the prior reservation (if any) until the flush
/// runs.
pub fn reserve_target(
    commands: &mut Commands,
    target: Entity,
    owner: Entity,
    tick: u64,
    ttl_ticks: u64,
) {
    commands
        .entity(target)
        .insert(Reserved::new(owner, tick, ttl_ticks));
}

/// Remove the `Reserved` component from `target`. Idempotent: if no
/// reservation exists, the operation is a no-op.
///
/// **Real-world effect** — schedules an ECS removal of `Reserved` on
/// `target`. Used by `plan_substrate::lifecycle::abandon_plan` and
/// terminal step-failure paths to release the cat's hold on the
/// resource so peers can re-pick it next tick.
pub fn release_target(commands: &mut Commands, target: Entity) {
    commands.entity(target).remove::<Reserved>();
}

/// Maintenance system: remove `Reserved` whose `expires_tick` is in
/// the past relative to the current sim tick. Bounds the world-size of
/// the marker so abandoned reservations (cats that crashed, plans that
/// weren't released cleanly, etc.) don't accumulate.
///
/// Registered in chain 2a's decay batch alongside `decay_grooming` and
/// friends — see `src/plugins/simulation.rs`.
pub fn expire_reservations(
    mut commands: Commands,
    time: Res<crate::resources::time::TimeState>,
    reserved: Query<(Entity, &Reserved)>,
) {
    let now = time.tick;
    for (entity, r) in reserved.iter() {
        if r.is_expired(now) {
            commands.entity(entity).remove::<Reserved>();
        }
    }
}
