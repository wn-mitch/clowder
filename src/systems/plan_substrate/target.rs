//! Target-handling operations: validity, carryover across step
//! boundaries, alive-eligibility filter. **072 stubs — bodies land in
//! 074** (`EligibilityFilter::require_alive` + step-resolver
//! `validate_target`). Today every function here returns the same
//! result the inline call site currently produces.

use bevy_ecs::prelude::*;

use crate::ai::dse::EligibilityFilter;
use crate::components::goap_plan::StepExecutionState;
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
