//! Ticket 327 — `fight_method`, the Combat chain's Tier-1 Live HTN
//! method.
//!
//! Catches the `engage_threat` label emitted by `WARRIORS_PATH`
//! milestones (`crate::ai::aspirations::combat::WARRIORS_PATH`). When
//! a cat with the Warrior's Path chain hits the L2 wrap site, the
//! picker produces an `AspirationEmissions` row whose label is
//! `engage_threat`; the L2 author replaces the default
//! `Intention::Activity { Idle }` wrap with `Intention::Goal {
//! engage_threat }`; 320's HTN frame-push gate looks up
//! `engage_threat` in `MethodRegistry`, finds *this* method `Live`,
//! and pushes one `GoalFrame` onto the cat's `HeldGoalStack`.
//!
//! # Scope
//!
//! This is the **combine-and-test slice**, mirroring `hunt_method`'s
//! 321 shape. Production gating (threat-in-range belief check,
//! engagement-distance predicate) is a later balance-thread refinement;
//! at 327 land the method is intentionally minimal:
//!
//! - `applicable_when: Live(always_true)` — fires on every cat that
//!   already reached the L2 wrap site via a WARRIORS_PATH emission.
//!   The picker's per-`Emit`-row `applicable_when` gate is also
//!   `always_true` at 327 land; a follow-on balance pass replaces both
//!   with threat-belief checks.
//! - One primitive sub-goal: `Action::Fight` with `TargetHint::Threat`.
//!   No multi-step decomposition; the existing Fight-DSE handles
//!   threat selection and step chaining.
//! - `failure_strategy: Abandon` — there are no sibling methods to
//!   backtrack to, and the picker re-emits next tick if the leaf
//!   abandons.
//! - `domain: Some(Combat)` — exposes the method to the picker's §H
//!   step-3 domain-affinity fallback for any Combat-chain milestone
//!   whose `emits[]` table is empty (e.g. SHADOW_FIGHTER, pending its
//!   own follow-on ticket).

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `fight_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn fight_method() -> Method {
    Method {
        id: MethodId("fight_method"),
        goal_label: "engage_threat",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "engage_threat",
            action: Action::Fight,
            target_hint: TargetHint::Threat,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Combat),
    }
}
