//! `plan_substrate` — unified plan-lifecycle API.
//!
//! Ticket 072 (parent: 071, planning-substrate hardening sub-epic).
//!
//! Plan-lifecycle ops (record-failure, abandon, preempt-cleanup,
//! step-target carryover, disposition switch) used to be inlined at
//! every call site in `goap.rs`. Each prior incident in the
//! "stuck-cat" bug class (tickets 038, 041, 027b's failed soak)
//! patched one inline site; identical bugs at sister sites went
//! uncovered until the next perturbing schedule change exposed them.
//!
//! This module lifts plan-lifecycle ops into a single, well-tested
//! surface. **072 is a strictly mechanical refactor** — every function
//! body is the inline code lifted verbatim, with no behavior change.
//! Behavior bodies (target-failure memory, dead-entity validation,
//! commitment-tenure modifier, last-resort promotion) land in
//! tickets 073–081 inside this module rather than scattering back
//! across `goap.rs` + step resolvers.
//!
//! ## Module layout
//!
//! - [`lifecycle`] — `record_step_failure`, `abandon_plan`, `try_preempt`.
//! - [`target`] — `validate_target`, `carry_target_forward`,
//!   `require_alive_filter`.
//! - [`disposition`] — `record_disposition_switch`.
//!
//! ## Why functions, not a system
//!
//! The migration sites in `goap.rs::resolve_goap_plans` already own
//! the surrounding `Commands` / `Query` / `EventLog` borrows.
//! Re-shaping them into a Bevy system would require flowing per-cat
//! state across system boundaries — a larger refactor than 072 wants.
//! The functions here are pure-ish helpers the existing systems call
//! on a per-cat basis.
//!
//! ## IAUS input keys
//!
//! Constants exposed here are the `&'static str` keys that downstream
//! tickets register Considerations under. 072 introduces the names so
//! migration sites and tests can reference them without each ticket
//! re-introducing a string literal.

pub mod disposition;
pub mod lifecycle;
pub mod sensors;
pub mod target;

pub use disposition::record_disposition_switch;
pub use lifecycle::{
    abandon_plan, record_step_failure, try_preempt, PreemptKind, PreemptOutcome,
};
pub use sensors::{cooldown_curve, prune_recent_target_failures, target_recent_failure_age_normalized};
pub use target::{
    carry_target_forward, expire_reservations, release_target, require_alive_and_unreserved_filter,
    require_alive_filter, require_unreserved_filter, reserve_target, validate_target,
    TargetInvalidReason,
};

// ---------------------------------------------------------------------------
// IAUS Consideration input keys
// ---------------------------------------------------------------------------
//
// Names are stable `&'static str` per the open-set marker contract
// (see `src/components/markers.rs`). Downstream tickets register
// Considerations against these keys; 072 introduces the names so the
// API surface is complete on landing.

/// 073 — `RecentTargetFailures` Consideration on all 6 target DSEs.
pub const TARGET_RECENT_FAILURE_INPUT: &str = "target_recent_failure";

/// 075 — `CommitmentTenure` Modifier (per-disposition tenure curve).
pub const COMMITMENT_TENURE_INPUT: &str = "commitment_tenure_progress";

/// 076 — `LastResortPromotion` Modifier (escalates after `recovery_failure_count`).
pub const RECOVERY_FAILURE_COUNT_INPUT: &str = "recovery_failure_count";

/// 078 — backport of 027b's `bond_score` pin to a `target_pairing_intention`
/// Consideration on `socialize_target.rs`.
pub const PAIRING_INTENTION_INPUT: &str = "target_pairing_intention";

#[cfg(test)]
mod tests;
