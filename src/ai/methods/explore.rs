//! Ticket 329 — `explore_method`, the Exploration chain's Tier-1
//! Live HTN method.
//!
//! Catches the `explore_territory` label emitted by both
//! MAPMAKER and BEYOND_THE_BORDER milestones
//! (`crate::ai::aspirations::exploration`). When a cat with either
//! Exploration chain hits the L2 wrap site, the picker produces an
//! `AspirationEmissions` row whose label is `explore_territory`;
//! 320's HTN frame-push gate looks up `explore_territory` in
//! `MethodRegistry`, finds *this* method `Live`, and pushes one
//! `GoalFrame`.
//!
//! # Scope
//!
//! Combine-and-test slice mirroring `fight_method`'s 327 shape. Both
//! Exploration chains share one Tier-1 method — the chains differ in
//! milestone gating (tile-discovery counts vs unique-region visits)
//! but in 329's combine-and-test land both reduce to `Action::Explore`.
//!
//! - `applicable_when: Live(always_true)` — fires on every cat at
//!   the wrap site.
//! - One primitive sub-goal: `Action::Explore` with
//!   `TargetHint::UnexploredTile`. The Explore-DSE handles
//!   unexplored-tile selection and step chaining.
//! - `failure_strategy: Abandon`.
//! - `domain: Some(Exploration)` — also reachable via §H step-3
//!   domain-affinity fallback.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `explore_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn explore_method() -> Method {
    Method {
        id: MethodId("explore_method"),
        goal_label: "explore_territory",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "explore_territory",
            action: Action::Explore,
            target_hint: TargetHint::UnexploredTile,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Exploration),
    }
}
