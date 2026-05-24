//! `courtship_method` — Live HTN method (#323).
//!
//! Mirrors 127's [`JointIntention`] `PracticeStage` advance for the
//! Courtship practice. Four ordered sub-goals match the four stages a
//! cat traverses while courting:
//!
//! 1. [`PracticeStage::CourtshipApproach`] → `approach_partner` —
//!    initial Socialize-with-partner contact.
//! 2. [`PracticeStage::CourtshipCourting`] → `allogroom_partner` —
//!    allogrooming the partner.
//! 3. [`PracticeStage::CourtshipMating`] → `mate_with_partner` — the
//!    existing single-action Mating dispatch (`Action::Mate`). #340
//!    upgrades this sub-goal to `SubGoal::Goal(GoalState {
//!    label: "mating_event_completed" })` so it recurses into the
//!    `mate_with_goal` method's four primitive chain, demonstrating
//!    registry-driven decomposition end-to-end.
//! 4. [`PracticeStage::CourtshipBonded`] → `consolidate_bonded` —
//!    continued partner-socializing maintenance. No dedicated `Bond` or
//!    `Pair` Action exists today (the joint_intention.rs module doc
//!    line 64 references an aspirational `Action::Pair` that was never
//!    added to [`crate::ai::Action`]); continued presence manifests
//!    as `Action::Socialize` with the partner as target. That matches
//!    today's PairingActivity-during-pregnancy behavior the Bonded
//!    stage was authored to subsume (per `PracticeStage::CourtshipBonded`
//!    doc).
//!
//! # Architecture
//!
//! This is the first Live HTN method that mirrors a 127 JointIntention
//! practice end-to-end. The L2 evaluator picks a Courtship-emitted
//! `Intention::Goal` (label `"courtship_completed"`) on a cat already
//! carrying [`JointIntention { practice: Courtship, .. }`], looks up
//! this method via [`crate::ai::methods::MethodRegistry::lookup`], and
//! pushes a [`crate::components::GoalFrame`] onto the cat's
//! [`crate::components::HeldGoalStack`].
//!
//! Stage progression stays owned by 127's [`crate::ai::joint_intention`]
//! author system — this method does not author `JointIntention.stage`
//! transitions itself. The method's `sub_goal_index` advances per the
//! L2 evaluator's primitive-leaf completion contract (320); the
//! [`JointIntention.stage`] field advances per the
//! [`crate::components::joint_intention::next_stage`] predicate. Both
//! observe the same ground-truth proxies (partner interaction tick,
//! bond tier, fertility phase, pregnancy), so the two advance in step
//! by construction. No separate stage-sync system is needed.
//!
//! `JointIntention` stays as the mutually-public projection (per
//! 127's §Semantic category — codified body language); the method
//! frame is the actor-private commitment that decomposes the practice
//! into executable primitives.
//!
//! # Dispatch wiring
//!
//! As of 323 land, the cat's per-tick `chosen_action` is still picked
//! by the L3 softmax over per-tick DSEs (Socialize-DSE / GroomOther-
//! DSE / Mate-DSE). The method frame on the [`HeldGoalStack`] carries
//! the commitment; the softmax picks the matching DSE because the
//! partner-bias multiplier (`joint_bias_multiplier`) already amplifies
//! the relevant resolvers' scores. The dispatch hookup that would
//! route execution through the method's primitives (DSE / plan
//! template / resolver) lands as a follow-on alongside #340's port,
//! analogous to #332 / #333's deferred dispatch pattern.

use crate::ai::dse::GoalState;
use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::joint_intention::{JointIntention, PracticeKind};
use crate::components::physical::Dead;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate — alive cat carrying a
/// [`JointIntention`] whose `practice` field equals
/// [`PracticeKind::Courtship`].
///
/// The 127 author system (`author_joint_intentions`) inserts the
/// Component on matched candidates and removes it on any
/// `JointDropBranch` trigger. When the partner-side drop cascades
/// (`PartnerLeftPractice`), this predicate flips false on the
/// following tick, and the L2 evaluator's frame-walker propagates
/// `MethodFailure::Abandon` via the method's failure strategy.
///
/// Dead cats can't court — the `Dead` Component is added by
/// `systems::death::handle_death_message` and persists until the
/// despawn pass, so the gate catches mid-tick deaths cleanly.
pub fn has_active_courtship(world: &World, entity: Entity) -> bool {
    let ent = world.entity(entity);
    if ent.contains::<Dead>() {
        return false;
    }
    ent.get::<JointIntention>()
        .is_some_and(|ji| ji.practice == PracticeKind::Courtship)
}

/// Construct the `courtship_method` literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn courtship_method() -> Method {
    const SUB_GOALS: &[SubGoal] = &[
        SubGoal::Primitive {
            label: "approach_partner",
            action: Action::Socialize,
            target_hint: TargetHint::Partner,
        },
        SubGoal::Primitive {
            label: "allogroom_partner",
            action: Action::GroomOther,
            target_hint: TargetHint::Partner,
        },
        // #340 recursion seam — the third sub-goal decomposes
        // into `mate_with_goal` via the method registry. When the
        // L2 evaluator advances `courtship_method` to
        // `sub_goal_index == 2`, it pushes a new `GoalFrame` for
        // `mate_with_goal` onto the cat's `HeldGoalStack`,
        // producing the two-deep frame stack that is the 128
        // worked-example payoff. Depth-cap (8) is trivially
        // satisfied (depth 2). The `achieved` predicate stays
        // `false` so completion flows through the natural
        // `sub_goal_index` advancement path; the
        // `failure_strategy: Abandon` on `mate_with_goal` handles
        // partner-loss / eligibility-loss as method abandonment
        // rather than premature achievement.
        SubGoal::Goal(GoalState::predicate("mating_event_completed", |_, _| false)),
        // Bonded-stage held action: continued partner-presence.
        // No dedicated `Bond` / `Pair` Action exists; the Bonded
        // stage's "post-conception or post-Mates-bond settled
        // state" (per `PracticeStage::CourtshipBonded` doc)
        // manifests as ongoing Socialize-with-partner today —
        // matching the PairingActivity-during-pregnancy behavior
        // 127 subsumes 1:1.
        SubGoal::Primitive {
            label: "consolidate_bonded",
            action: Action::Socialize,
            target_hint: TargetHint::Partner,
        },
    ];
    Method {
        id: MethodId("courtship_method"),
        goal_label: "courtship_completed",
        applicable_when: ApplicableWhen::Live(has_active_courtship),
        sub_goals: SUB_GOALS,
        // Abandon: no sibling Courtship methods today, and a
        // `JointDropBranch` trigger (partner Dead, BondLost, cascade,
        // …) propagates as practice-abandon, not method-backtrack.
        // The L2 evaluator drops the GoalFrame and returns control to
        // the picker. Matches `hunt_method` / `fight_method` /
        // `flee_method`'s choice for single-method goals.
        failure_strategy: MethodFailure::Abandon,
        // Courtship is reactive substrate driven by 127's matchmaker
        // (`author_joint_intentions` inserts `JointIntention` on
        // matched candidates), not aspirational achievement. No
        // `AspirationDomain` matches it — the picker's §H step-3
        // domain-affinity fallback at `aspiration_picker.rs:349` is
        // not the emission path. Same rationale as `rear_kitten` /
        // `mourn_at_grave` (both `domain: None`).
        //
        // Setting `Social` here would expose the method to the
        // domain-affinity fallback for any Social-chain milestone
        // with an empty `emits[]` table — the L2 author would then
        // push a `GoalFrame` for `courtship_method` on courtship-
        // active cats, the multi-step frame-pin at
        // `goap.rs:2841` would fire (`sub_goal_count == 4 > 1`),
        // and `htn_primitive_actions` would panic on `Action::Socialize`
        // (the function only handles the kitten + mourn arcs at
        // 323 land). Explicit emission of `courtship_completed`
        // lands as a follow-on once the dispatch hookup (DSE / plan
        // template / resolver) extends `htn_primitive_actions` to
        // cover the Social actions.
        domain: None,
    }
}
