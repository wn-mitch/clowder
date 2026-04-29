//! Target-handling operations: validity, carryover across step
//! boundaries, alive-eligibility filter, resource reservation. 072
//! stubbed `validate_target` / `require_alive_filter`; 074 fills those
//! bodies. Ticket 080 adds the resource-reservation API
//! (`reserve_target` / `release_target` / `require_unreserved_filter`).

use bevy_ecs::prelude::*;

use crate::ai::dse::EligibilityFilter;
use crate::components::goap_plan::StepExecutionState;
use crate::components::reserved::Reserved;
use crate::components::RecentTargetFailures;

// ---------------------------------------------------------------------------
// Target validity (074)
// ---------------------------------------------------------------------------

/// Why a target entity is invalid. Currently only one variant
/// (`Despawned`); 074 may add `Reserved` when ticket 080 lands the
/// resource-reservation eligibility check. 072 ships the enum so
/// callers (step resolvers) can branch on it without each ticket
/// re-introducing the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetInvalidReason {
    /// Entity has been despawned since the plan committed to it.
    Despawned,
}

/// Placeholder query type for target validity. 074 expands this into
/// a `SystemParam` bundling the queries needed to check despawn /
/// reservation. 072 is opaque — the stub doesn't read it, so the
/// type is a unit struct callers can construct freely.
#[derive(Debug, Default)]
pub struct TargetValidityQuery;

/// Validate that `target` is still a usable entity for the calling
/// step. **072 stub — always returns `Ok(())`.** 074 implements the
/// dead-entity check (`World::contains_entity` or similar) and the
/// `Reserved` check from ticket 080.
///
/// Step resolvers in `src/steps/disposition/*.rs` and
/// `src/steps/building/*.rs` will call this at entry once 074 lands.
/// 072 lifts the call signature; the bodies of those resolvers don't
/// change behavior because the stub is unconditionally `Ok`.
pub fn validate_target(
    _target: Entity,
    _validity: &TargetValidityQuery,
) -> Result<(), TargetInvalidReason> {
    Ok(())
}

// ---------------------------------------------------------------------------
// carry_target_forward
// ---------------------------------------------------------------------------

/// Carry a step's `target_entity` forward from the prior step when
/// the current step's `target_entity` is `None`. Lifted verbatim from
/// `goap.rs:2817–2820` — the `EngagePrey` carryover that copies
/// `step_state[step_idx - 1].target_entity` into
/// `step_state[step_idx].target_entity` when the latter is `None`.
///
/// 072 preserves the existing unconditional copy. 074 will add a
/// `validate_target` check before the copy so dead targets are
/// rejected (and 073's `RecentTargetFailures` notes the dead target).
///
/// **Real-world effect** — when `step_state[step_idx].target_entity`
/// is `None` and `step_idx > 0`, copies the prior step's
/// `target_entity` into the current step. Returns the resulting
/// target entity (or `None` if neither slot held one).
pub fn carry_target_forward(
    step_state: &mut [StepExecutionState],
    step_idx: usize,
    _validity: &TargetValidityQuery,
    _recent: Option<&mut RecentTargetFailures>,
) -> Option<Entity> {
    if step_state[step_idx].target_entity.is_none() && step_idx > 0 {
        step_state[step_idx].target_entity = step_state[step_idx - 1].target_entity;
    }
    step_state[step_idx].target_entity
}

// ---------------------------------------------------------------------------
// require_alive_filter (074 — IAUS engine extension)
// ---------------------------------------------------------------------------

/// Build an `EligibilityFilter` that requires the candidate target's
/// alive marker. **072 stub — returns `EligibilityFilter::new()`** (an
/// always-pass filter), so today this is identical to "no filter at
/// all". 074 implements the real version: a marker key the
/// target-DSE registration adds via `EligibilityFilter::require(...)`.
///
/// Exposed here so 074 doesn't have to introduce a new public symbol;
/// the call sites can land their `require_alive_filter()` calls today
/// (no behavior change) and 074 fills in the body.
pub fn require_alive_filter() -> EligibilityFilter {
    EligibilityFilter::new()
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
