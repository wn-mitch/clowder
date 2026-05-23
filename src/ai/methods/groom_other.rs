//! Ticket 326 — `groom_other_method`, the Social chain's Tier-1 Live
//! HTN method for the THE_BELOVED arc.
//!
//! Catches the `groom_other` label emitted by every THE_BELOVED
//! milestone (`crate::ai::aspirations::social`). When a cat with the
//! THE_BELOVED chain hits the L2 wrap site, the picker produces an
//! `AspirationEmissions` row whose label is `groom_other`; 320's HTN
//! frame-push gate looks up `groom_other` in `MethodRegistry`, finds
//! *this* method `Live`, and pushes one `GoalFrame`.
//!
//! # Scope
//!
//! Combine-and-test slice mirroring `build_method`'s 330 shape. The
//! THE_BELOVED milestones gate on `ActionCount { GroomOther }` and
//! `FormBond { Partners }` thresholds; in 326's combine-and-test land
//! they all reduce to `Action::GroomOther` against the cat's
//! grooming-target picker (`src/ai/dses/groom_other_target.rs`).
//! HEART_OF_THE_COLONY routes through the sibling `socialize_method`
//! instead.
//!
//! - `applicable_when: Live(always_true)`.
//! - One primitive sub-goal: `Action::GroomOther` with
//!   `TargetHint::GroomingTarget`. The GroomOther-DSE handles target
//!   selection and step chaining.
//! - `failure_strategy: Abandon`.
//! - `domain: Some(Social)` — also reachable via §H step-3
//!   domain-affinity fallback.

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `groom_other_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn groom_other_method() -> Method {
    Method {
        id: MethodId("groom_other_method"),
        goal_label: "groom_other",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "groom_other",
            action: Action::GroomOther,
            target_hint: TargetHint::GroomingTarget,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Social),
    }
}
