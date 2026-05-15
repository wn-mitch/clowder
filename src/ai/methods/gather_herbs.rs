//! Ticket 328 — `gather_herbs_method`, the Herbcraft chain's
//! apprentice-side Tier-1 Live HTN method.
//!
//! Catches the `gather_herbs` label emitted by the
//! WHISKERWEAVERS_APPRENTICE chain
//! (`crate::ai::aspirations::herbcraft::WHISKERWEAVERS_APPRENTICE`).
//! When a cat with the Apprentice chain hits the L2 wrap site, the
//! picker produces an `AspirationEmissions` row whose label is
//! `gather_herbs`; 320's HTN frame-push gate looks up `gather_herbs`
//! in `MethodRegistry`, finds *this* method `Live`, and pushes one
//! `GoalFrame` onto the cat's `HeldGoalStack`.
//!
//! # Scope
//!
//! Combine-and-test slice mirroring `fight_method`'s 327 shape.
//! Production gating (herb-believed-in-range, basket-not-full) lands
//! in a follow-on balance pass; at 328 land the method is intentionally
//! minimal:
//!
//! - `applicable_when: Live(always_true)` — fires on every cat that
//!   already reached the L2 wrap site via an Apprentice emission.
//! - One primitive sub-goal: `Action::HerbcraftGather` with
//!   `TargetHint::Herb`. The Herbcraft-Gather DSE handles herb-tile
//!   selection and step chaining.
//! - `failure_strategy: Abandon` — no sibling backtrack methods at
//!   this label.
//! - `domain: Some(Herbcraft)` — exposes the method to the picker's
//!   §H step-3 domain-affinity fallback for any Herbcraft milestone
//!   whose `emits[]` table is empty.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `gather_herbs_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn gather_herbs_method() -> Method {
    Method {
        id: MethodId("gather_herbs_method"),
        goal_label: "gather_herbs",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "gather_herbs",
            action: Action::HerbcraftGather,
            target_hint: TargetHint::Herb,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Herbcraft),
    }
}
