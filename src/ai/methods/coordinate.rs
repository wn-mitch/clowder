//! Ticket 331 — `coordinate_method`, the Leadership chain's Tier-1
//! Live HTN method.
//!
//! Catches the `direct_colony` label emitted by both
//! VOICE_OF_THE_COLONY and THE_UNIFIER milestones
//! (`crate::ai::aspirations::leadership`). When a cat with either
//! Leadership chain hits the L2 wrap site, the picker produces an
//! `AspirationEmissions` row whose label is `direct_colony`; 320's
//! HTN frame-push gate looks up `direct_colony` in `MethodRegistry`,
//! finds *this* method `Live`, and pushes one `GoalFrame`.
//!
//! # Scope
//!
//! Combine-and-test slice mirroring `fight_method`'s 327 shape. The
//! chains differ in milestone gating (Coordinate-count for VOICE,
//! mixed Socialize/Coordinate/Mentor for UNIFIER) but in 331's
//! combine-and-test land both reduce to `Action::Coordinate`.
//! Coordinator-directive integration (#335 territory) is out of
//! scope here.
//!
//! - `applicable_when: Live(always_true)`.
//! - One primitive sub-goal: `Action::Coordinate` with
//!   `TargetHint::Audience`. The Coordinate-DSE handles audience
//!   selection and step chaining.
//! - `failure_strategy: Abandon`.
//! - `domain: Some(Leadership)` — also reachable via §H step-3
//!   domain-affinity fallback.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `coordinate_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn coordinate_method() -> Method {
    Method {
        id: MethodId("coordinate_method"),
        goal_label: "direct_colony",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "direct_colony",
            action: Action::Coordinate,
            target_hint: TargetHint::Audience,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Leadership),
    }
}
