//! Ticket 347 — `patrol_method`, the Combat chain's Patrol-based
//! Tier-1 Live HTN method.
//!
//! Catches the `patrol_route` label emitted by SHADOW_FIGHTER
//! milestones (`crate::ai::aspirations::combat::SHADOW_FIGHTER`).
//! When a cat with the Shadow Fighter chain hits the L2 wrap site,
//! the picker produces an `AspirationEmissions` row whose label is
//! `patrol_route`; 320's HTN frame-push gate looks up `patrol_route`
//! in `MethodRegistry`, finds *this* method `Live`, and pushes one
//! `GoalFrame`.
//!
//! # Scope
//!
//! Mirror of `fight_method`'s 327 combine-and-test shape, but for the
//! Combat domain's *Patrol*-based chain rather than the Fight-based
//! WARRIORS_PATH. Together with the already-Live `flee_method`
//! (re-used as SHADOW_FIGHTER's Tertiary survival fallback), 347
//! finishes Combat-domain wiring. Production gating (perimeter-
//! unwatched predicate) lands in a follow-on balance pass.
//!
//! - `applicable_when: Live(always_true)`.
//! - One primitive sub-goal: `Action::Patrol` with
//!   `TargetHint::PatrolRoute`. The Patrol-DSE handles route selection
//!   and step chaining.
//! - `failure_strategy: Abandon`.
//! - `domain: Some(Combat)` — also reachable via §H step-3
//!   domain-affinity fallback alongside `fight_method` / `flee_method`.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `patrol_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn patrol_method() -> Method {
    Method {
        id: MethodId("patrol_method"),
        goal_label: "patrol_route",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "patrol_route",
            action: Action::Patrol,
            target_hint: TargetHint::PatrolRoute,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Combat),
    }
}
