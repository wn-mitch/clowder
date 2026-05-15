//! Ticket 328 — `prepare_remedy_method`, the Herbcraft chain's
//! healer-side Tier-1 Live HTN method.
//!
//! Catches the `prepare_remedy` label emitted by the HEALERS_CALLING
//! chain (`crate::ai::aspirations::herbcraft::HEALERS_CALLING`). When
//! a cat with the Healer chain hits the L2 wrap site, the picker
//! produces an `AspirationEmissions` row whose label is
//! `prepare_remedy`; 320's HTN frame-push gate looks up
//! `prepare_remedy` in `MethodRegistry`, finds *this* method `Live`,
//! and pushes one `GoalFrame` onto the cat's `HeldGoalStack`.
//!
//! # Scope
//!
//! Mirror of `gather_herbs_method`'s combine-and-test shape. The
//! semantic split (apprentice gathers / healer remedies) reflects
//! the chains' narrative metaphors — both run `Action::Herbcraft*`
//! sub-actions, so chain-level routing is what distinguishes them.
//! Production gating (patient-believed-in-need, remedy-stock-on-hand)
//! lands in a follow-on balance pass.
//!
//! - `applicable_when: Live(always_true)`.
//! - One primitive sub-goal: `Action::HerbcraftRemedy` with
//!   `TargetHint::Patient`. The Herbcraft-Remedy DSE handles patient
//!   selection and step chaining.
//! - `failure_strategy: Abandon`.
//! - `domain: Some(Herbcraft)` — also reachable via §H step-3
//!   domain-affinity fallback alongside `gather_herbs_method`.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `prepare_remedy_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn prepare_remedy_method() -> Method {
    Method {
        id: MethodId("prepare_remedy_method"),
        goal_label: "prepare_remedy",
        applicable_when: ApplicableWhen::Live(|_world, _entity| true),
        sub_goals: &[SubGoal::Primitive {
            label: "prepare_remedy",
            action: Action::HerbcraftRemedy,
            target_hint: TargetHint::Patient,
        }],
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Herbcraft),
    }
}
