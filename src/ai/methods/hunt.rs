//! Ticket 321 — `hunt_method`, the combine-and-test slice's Live HTN
//! method.
//!
//! Catches the `hunt_prey` label emitted by Hunting's "First Blood"
//! milestone (`crate::ai::aspirations::hunting::MASTER_OF_THE_HUNT`).
//! When a cat with the Hunting chain at milestone 0 hits the L2 wrap
//! site, the picker produces an `AspirationEmissions` row whose label
//! is `hunt_prey`; the L2 author replaces the default
//! `Intention::Activity { Idle }` wrap with `Intention::Goal {
//! hunt_prey }`; 320's HTN frame-push gate looks up `hunt_prey` in
//! `MethodRegistry` and finds *this* method `Live`, pushing one
//! `GoalFrame` onto the cat's `HeldGoalStack` and firing
//! `Feature::MethodAdopted`.
//!
//! # Scope
//!
//! This is the **combine-and-test slice**, not the final production
//! `hunt_method`. The full Hunting wrapper (#325) authors the
//! production gating predicate (prey-in-range belief check) and the
//! multi-step primitive chain (stalk → pounce → engage). At 321 land
//! the method is intentionally minimal:
//!
//! - `applicable_when: Live(always_true)` — fires on every cat with
//!   the Hunting chain at milestone 0. The picker's per-`Emit`-row
//!   `applicable_when` gate is also `always_true` at 321 land; #325
//!   replaces both with prey-in-range belief checks.
//! - One primitive sub-goal: `Action::Hunt` with `TargetHint::Prey`.
//!   No multi-step decomposition; the existing Hunt-DSE handles
//!   target selection and step chaining.
//! - `failure_strategy: Abandon` — there are no sibling methods to
//!   backtrack to, and the picker re-emits next tick if the leaf
//!   abandons.
//! - `domain: Some(Hunting)` — exposes the method to the picker's
//!   §H step-3 domain-affinity fallback for any Hunting-chain
//!   milestone whose `emits[]` table is empty.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `hunt_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn hunt_method() -> Method {
    Method {
        id: MethodId("hunt_method"),
        goal_label: "hunt_prey",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "pursue_prey",
            action: Action::Hunt,
            target_hint: TargetHint::Prey,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Hunting),
    }
}
