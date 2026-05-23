//! Ticket 326 — `socialize_method`, the Social chain's Tier-1 Live
//! HTN method for the HEART_OF_THE_COLONY arc.
//!
//! Catches the `socialize` label emitted by every HEART_OF_THE_COLONY
//! milestone (`crate::ai::aspirations::social`). When a cat with the
//! HEART_OF_THE_COLONY chain hits the L2 wrap site, the picker produces
//! an `AspirationEmissions` row whose label is `socialize`; 320's HTN
//! frame-push gate looks up `socialize` in `MethodRegistry`, finds
//! *this* method `Live`, and pushes one `GoalFrame`.
//!
//! # Scope
//!
//! Combine-and-test slice mirroring `build_method`'s 330 shape. The
//! HEART_OF_THE_COLONY milestones gate on `FormBond { Friends }` and
//! `ActionCount { Socialize }` thresholds; in 326's combine-and-test
//! land they all reduce to `Action::Socialize` against the cat's
//! socialize-target picker (`src/ai/dses/socialize_target.rs`). THE_BELOVED
//! routes through the sibling `groom_other_method` instead.
//!
//! - `applicable_when: Live(always_true)`.
//! - One primitive sub-goal: `Action::Socialize` with
//!   `TargetHint::SocialPartner`. The Socialize-DSE handles partner
//!   selection and step chaining.
//! - `failure_strategy: Abandon`.
//! - `domain: Some(Social)` — also reachable via §H step-3
//!   domain-affinity fallback.

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `socialize_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn socialize_method() -> Method {
    Method {
        id: MethodId("socialize_method"),
        goal_label: "socialize",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "socialize",
            action: Action::Socialize,
            target_hint: TargetHint::SocialPartner,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Social),
    }
}
