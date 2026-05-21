//! Ticket 327 — `flee_method`, the Combat chain's survival-fallback
//! Tier-1 Live HTN method.
//!
//! Catches the `flee_to_safety` label emitted by `WARRIORS_PATH`
//! milestones as their Tertiary `Emit` row — the survival fallback for
//! Combat-aspiring cats whose situation no longer favours engaging.
//! When a cat with the Warrior's Path chain hits the L2 wrap site and
//! the Primary `engage_threat` emit isn't selected, the Tertiary
//! `flee_to_safety` row catches and the L2 author replaces the default
//! `Intention::Activity { Idle }` wrap with `Intention::Goal {
//! flee_to_safety }`; 320's HTN frame-push gate looks up
//! `flee_to_safety` in `MethodRegistry`, finds *this* method `Live`,
//! and pushes one `GoalFrame`.
//!
//! # Scope
//!
//! Mirror of `fight_method`'s combine-and-test shape. Production
//! gating (health-deficit / wounded predicate, safety-axis threshold)
//! is a later balance-thread refinement; at 327 land the method is
//! intentionally minimal:
//!
//! - `applicable_when: Live(always_true)` — every cat at this label is
//!   already an L2-wrap candidate via a WARRIORS_PATH emission. A
//!   follow-on balance pass tightens this with a wounded-cat predicate.
//! - One primitive sub-goal: `Action::Flee` with `TargetHint::SafeGround`.
//!   The existing Flee-DSE handles safe-tile selection and step
//!   chaining.
//! - `failure_strategy: Abandon` — no sibling backtrack methods.
//! - `domain: Some(Combat)` — exposes the method to the picker's §H
//!   step-3 domain-affinity fallback for any Combat-chain milestone
//!   whose `emits[]` table is empty.

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `flee_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn flee_method() -> Method {
    Method {
        id: MethodId("flee_method"),
        goal_label: "flee_to_safety",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "flee_to_safety",
            action: Action::Flee,
            target_hint: TargetHint::SafeGround,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Combat),
    }
}
