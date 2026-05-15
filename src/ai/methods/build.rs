//! Ticket 330 — `build_method`, the Building chain's Tier-1 Live
//! HTN method.
//!
//! Catches the `construct` label emitted by both DEN_SHAPER and
//! THE_ARCHITECT milestones (`crate::ai::aspirations::building`).
//! When a cat with either Building chain hits the L2 wrap site, the
//! picker produces an `AspirationEmissions` row whose label is
//! `construct`; 320's HTN frame-push gate looks up `construct` in
//! `MethodRegistry`, finds *this* method `Live`, and pushes one
//! `GoalFrame`.
//!
//! # Scope
//!
//! Combine-and-test slice mirroring `fight_method`'s 327 shape. The
//! chains differ in milestone gating (build-count thresholds vs
//! skill-level checkpoints) but in 330's combine-and-test land both
//! reduce to `Action::Build`. Strategist-coordinator alignment
//! (#335 territory) is out of scope here.
//!
//! - `applicable_when: Live(always_true)`.
//! - One primitive sub-goal: `Action::Build` with
//!   `TargetHint::ConstructionSite`. The Build-DSE handles
//!   construction-site selection and step chaining.
//! - `failure_strategy: Abandon`.
//! - `domain: Some(Building)` — also reachable via §H step-3
//!   domain-affinity fallback.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `build_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn build_method() -> Method {
    Method {
        id: MethodId("build_method"),
        goal_label: "construct",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "construct",
            action: Action::Build,
            target_hint: TargetHint::ConstructionSite,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Building),
    }
}
