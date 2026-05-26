//! `play_bout_method` — Live HTN method (#276).
//!
//! Mirrors 127's [`JointIntention`] `PracticeStage` advance for the
//! PlayBout practice. Three ordered sub-goals match the three stages a
//! cat traverses while play-bouting.
//!
//! - `PlayBoutApproach` → `approach_play_partner` — Socialize-with-
//!   partner contact bringing the cats together.
//! - `PlayBoutBouting` → `play_with_partner` — sustained Socialize with
//!   the partner. Commit B's Bouting-stage cascade (mood-lift to
//!   nearby witnesses plus narrative entry) fires here, replacing the
//!   legacy `on_play_initiated` observer at
//!   `personality_events.rs:266-401`.
//! - `PlayBoutCooldown` → `cool_down_after_play` — continued
//!   co-presence until `should_drop_joint` fires
//!   `JointDropBranch::Completed`, emitting
//!   `EventKind::JointPlayBoutCompleted` to the EventLog.
//!
//! # Architecture
//!
//! Follows the [`crate::ai::methods::courtship::courtship_method`]
//! pattern. Stage progression is owned by 127's
//! [`crate::ai::joint_intention`] author system — this method does not
//! author `JointIntention.stage` transitions itself. All three
//! sub-goals dispatch to `Action::Socialize` with `TargetHint::Partner`
//! (no bespoke action variant). The substrate-side novelty is the
//! JointIntention carrier; the Action-level continuity remains
//! Socialize.
//!
//! # Ticket 276 design rationale
//!
//! Per CLAUDE.md design pillar #2 (substrate over hacks), this method
//! retires the four-AND × RNG direct-emit at
//! `personality_events.rs:80-90`. Hosting the `play` continuity
//! canary on JointIntention makes "playing together"
//! mutually-public practice state (the ticket-127 semantic category),
//! co-extensive with the sister practices 274 (co-mentoring) and 275
//! (joint cache-stocking).
//!
//! # Dispatch wiring
//!
//! Same shape as `courtship_method` — the L3 softmax picks
//! `Action::Socialize` for cats holding a PlayBout `JointIntention`
//! because the partner-bias multiplier amplifies the relevant
//! Socialize resolver's score (when the resolver target matches
//! `JointIntention.partner`). The method frame on the
//! [`crate::components::HeldGoalStack`] carries the practice-level
//! commitment.

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::joint_intention::{JointIntention, PracticeKind};
use crate::components::physical::Dead;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate — alive cat carrying a
/// [`JointIntention`] whose `practice` field equals
/// [`PracticeKind::PlayBout`].
///
/// Mirrors `has_active_courtship` in
/// [`crate::ai::methods::courtship`]. When the 127 author system
/// removes the JI on any `JointDropBranch` trigger (including
/// `Completed` for PlayBout's natural Cooldown→done transition), this
/// predicate flips false and the L2 evaluator's frame-walker
/// propagates `MethodFailure::Abandon`.
pub fn has_active_playbout(world: &World, entity: Entity) -> bool {
    let ent = world.entity(entity);
    if ent.contains::<Dead>() {
        return false;
    }
    ent.get::<JointIntention>()
        .is_some_and(|ji| ji.practice == PracticeKind::PlayBout)
}

/// Construct the `play_bout_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn play_bout_method() -> Method {
    const SUB_GOALS: &[SubGoal] = &[
        SubGoal::Primitive {
            label: "approach_play_partner",
            action: Action::Socialize,
            target_hint: TargetHint::Partner,
        },
        SubGoal::Primitive {
            label: "play_with_partner",
            action: Action::Socialize,
            target_hint: TargetHint::Partner,
        },
        SubGoal::Primitive {
            label: "cool_down_after_play",
            action: Action::Socialize,
            target_hint: TargetHint::Partner,
        },
    ];
    Method {
        id: MethodId("play_bout_method"),
        goal_label: "play_bout_completed",
        applicable_when: ApplicableWhen::Live(has_active_playbout),
        sub_goals: SUB_GOALS,
        // Abandon: no sibling PlayBout methods today, and a
        // `JointDropBranch` trigger (PartnerInvalid, Completed, etc.)
        // propagates as practice-abandon, not method-backtrack.
        failure_strategy: MethodFailure::Abandon,
        // PlayBout is reactive substrate driven by 127's matchmaker
        // (`author_joint_intentions` inserts `JointIntention` on
        // matched candidates). Not aspirational achievement — no
        // `AspirationDomain` matches it. Same rationale as
        // `courtship_method` / `rear_kitten` / `mourn_at_grave`.
        domain: None,
    }
}
