//! `mate_with_goal` — Live HTN method (#340).
//!
//! Ports the §7.M three-step Mating chain
//! (`Socialize → GroomOther → Mate` against a partner) onto the HTN
//! method registry. The hand-coded `build_mating_chain` at
//! `src/systems/disposition.rs::build_mating_chain` lived inside the
//! unscheduled `disposition_to_chain` function — dead code at runtime
//! (the live mating path runs through the GOAP planner's
//! [`crate::ai::planner::actions::mating_actions`]). 340 makes that
//! dead-code observation surface-level: the method registry is now
//! the single inspectable home for the chain shape, even though the
//! per-tick `Action::Mate` dispatch path stays exactly where it is.
//!
//! # Worked example payoff
//!
//! Composed with #323's `courtship_method` (the outer Courtship arc),
//! the cat's `HeldGoalStack` carries a two-deep method frame on a
//! courting cat during the Mating stage:
//!
//! ```text
//! depth 0: courtship_method (sub_goal_index = 2 → mate_with_partner)
//! depth 1: mate_with_goal   (sub_goal_index = N → primitive)
//! ```
//!
//! That recursion is the 128 epic's worked-example screenshot:
//! registry-driven decomposition end-to-end, with the trace surface
//! making the structural commitment legible via
//! [`crate::resources::trace_log`]. The seam is the
//! `SubGoal::Goal(GoalState { label: "mating_event_completed" })`
//! upgrade in `courtship.rs`'s third sub-goal — applied in this
//! ticket once `mate_with_goal` registers.
//!
//! # Sub-goal shape
//!
//! Three primitives instead of the htn-methods.md §Worked-example's
//! aspirational four. The fourth primitive in the doc (`Action::Navigate`,
//! the explicit approach step) doesn't exist in [`crate::ai::Action`]
//! today, and authoring it would shadow the implicit travel injection
//! that
//! [`crate::ai::planner::actions::htn_primitive_actions`](crate::ai::planner::actions::htn_primitive_actions)
//! already does (it unions `travel_actions(distances)` with the
//! single Pattern-B leaf step). With travel implicit, three explicit
//! primitives suffice to preserve the hand-coded chain's behavior 1:1:
//!
//! 1. `socialize_with_partner` → `Action::Socialize` with the partner
//!    as target.
//! 2. `groom_partner` → `Action::GroomOther` with the partner as
//!    target.
//! 3. `complete_mating` → `Action::Mate`, the actual mating-event
//!    leaf the existing `resolve_mate_with` resolver consumes.
//!
//! # Dispatch wiring
//!
//! As of 340 land, the cat's per-tick `chosen_action` is still picked
//! by the L3 softmax / GOAP planner via `mating_actions`. The method
//! frame on the [`HeldGoalStack`] makes the commitment inspectable
//! (`just inspect`, L3 trace's `method_stack`); it does not yet route
//! execution. The dispatch follow-on extending
//! [`htn_primitive_actions`](crate::ai::planner::actions::htn_primitive_actions)
//! to the Social actions (Socialize / GroomOther / Mate) is the seam
//! that flips this method from observation-only to execution-driving
//! — same deferred-dispatch pattern as `rear_kitten` (#333) and
//! `caretake_kitten` (#398 Phase 1a).

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::markers::HasEligibleMate;
use crate::components::physical::Dead;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate — alive cat carrying the
/// [`HasEligibleMate`] marker.
///
/// The marker is authored by
/// [`crate::systems::mating::update_mate_eligibility_markers`] (per
/// #027 Bug 2) on cats that have a partner candidate matching the
/// matchmaker's reproductive-eligibility gate. It's the same gate the
/// existing GOAP planner consults via `mating_actions`'s
/// `ZoneIs(SocialTarget)` precondition shape, so this method mirrors
/// the live mating path's eligibility check 1:1.
///
/// Dead cats are filtered explicitly so a mid-tick `Dead` insertion
/// (before the despawn pass) doesn't leave the method applicable on a
/// corpse.
pub fn has_eligible_mate(world: &World, entity: Entity) -> bool {
    let ent = world.entity(entity);
    !ent.contains::<Dead>() && ent.contains::<HasEligibleMate>()
}

/// Construct the `mate_with_goal` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn mate_with_goal() -> Method {
    Method {
        id: MethodId("mate_with_goal"),
        goal_label: "mating_event_completed",
        applicable_when: ApplicableWhen::Live(has_eligible_mate),
        sub_goals: &[
            SubGoal::Primitive {
                label: "socialize_with_partner",
                action: Action::Socialize,
                target_hint: TargetHint::Partner,
            },
            SubGoal::Primitive {
                label: "groom_partner",
                action: Action::GroomOther,
                target_hint: TargetHint::Partner,
            },
            SubGoal::Primitive {
                label: "complete_mating",
                action: Action::Mate,
                target_hint: TargetHint::Partner,
            },
        ],
        // Abandon per #340 scope: no sibling methods share
        // `mating_event_completed`, and any `HasEligibleMate` loss
        // (partner-side cascade, repro-window close, conception)
        // propagates as method-abandon, not method-backtrack.
        failure_strategy: MethodFailure::Abandon,
        // Reactive substrate (driven by `HasEligibleMate` + the outer
        // `courtship_method`'s `SubGoal::Goal` recursion), not
        // aspirational achievement. `domain: None` keeps this method
        // out of the §H step-3 domain-affinity fallback at
        // `aspiration_picker.rs:349`, matching #323's `courtship_method`
        // / #333's `rear_kitten` / #332's `mourn_at_grave` pattern.
        // Setting a domain here would expose the method to fallback
        // emission paths, pushing a `GoalFrame` whose multi-step pin
        // would panic against today's `htn_primitive_actions` (which
        // doesn't cover `Action::Socialize` / `GroomOther` / `Mate`
        // at 340 land). Emission lands as a follow-on with the
        // dispatch-wiring extension to `htn_primitive_actions`.
        domain: None,
    }
}
