//! `rear_kitten` — Live HTN method (#333).
//!
//! Multi-stage kitten-rearing arc decomposed into wean → teach →
//! release, keyed to the existing
//! [`KittenDependency`](crate::components::KittenDependency)
//! Component on the kitten side. The mother-side commitment is a
//! `HeldGoalStack` frame (one per dependent kitten); no new
//! `RearKittenIntent` Component is introduced — `KittenDependency`
//! already carries the durable `mother: Option<Entity>` link, and
//! sibling kittens get sibling frames keyed by the kitten Entity
//! payload (per `GoalFrame.target`).
//!
//! # Status
//!
//! `applicable_when: Live` — the method registers Live in
//! `MethodRegistry`. Its `applicable_when` predicate gates on
//! "any kitten Entity carries `KittenDependency.mother == Some(self)`",
//! so the method is selectable only for queens (and the rare adoptive
//! parent) currently rearing dependent kittens.
//!
//! **Dispatch wiring is pending** — the cat's `chosen_action` is
//! still picked by the per-tick DSE softmax (Caretake covers the per-
//! tick "feed the kitten" leaf), not by the HTN method's primitive
//! sub-goals. The follow-on dispatch ticket (named in #333's landing
//! Log) wires DSE / GoapActionKind / plan template / resolver call
//! site so the cat's behavior advances Wean → Teach → Release
//! milestones based on the kitten's `KittenDependency.maturity`.
//!
//! # Why no `RearKittenIntent` substrate
//!
//! The relationship is already substrate: `KittenDependency.mother`
//! on the kitten is the durable, mutually-public link. Adding a
//! mother-side `RearKittenIntent` Component would duplicate the same
//! information without earning its keep — the §4.7 substrate-vs-
//! search-state classifier would flag it as additive substrate the
//! reverse-lookup already covers. The HTN method frame on the
//! mother's `HeldGoalStack` carries the *commitment*; the
//! relationship carries the *fact*.

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::markers::{HasJuvenileDependent, Parent};
use crate::components::physical::Dead;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate — alive cat carrying `Parent` AND
/// `HasJuvenileDependent`.
///
/// **Two-window emit gate (ticket 395).** The arc emits in two narrow
/// maturity windows, controlled by `HasJuvenileDependent`:
/// - **Early window** `[0, teach_done_threshold)` — Wean and Teach
///   milestones have eligible kittens.
/// - **Near-mature window** `[release_threshold, 1.0)` — Release is
///   pickable AND the kitten has not yet been symbolically released
///   (no `RearKittenReleased` marker).
///
/// Between the two windows, the parent does Caretake (kitten-side
/// `With<KittenDependency>` filter) and other DSEs — the rear_kitten
/// arc doesn't churn there. The `Parent` marker alone is too coarse
/// (stays true through natural maturity 1.0), which would per-tick
/// re-emit the arc and re-witness `Feature::KittenReleased` ~9000×
/// per kitten — see 395's Context for the analysis.
///
/// **Symmetric — both parents pitch in.** 395 retired the 333/364
/// mother-only deferral. The picker now matches kittens where this
/// cat is mother *or* father; whichever parent gets to the kitten
/// first witnesses each milestone, and `RearKittenReleased` blocks
/// the second parent's frame from re-firing Release.
///
/// **Ticket 397 Layer 3 — reactive-emit yield rule retired.** The
/// 395 yield clause (`!IsParentOfHungryKitten`) was a stand-in for
/// the deferred §L2.10.6 composition: it suppressed the entire arc
/// emit when a dependent kitten was hungry, so Caretake could win
/// softmax unopposed. With 397's pin-side guard at
/// `src/systems/goap.rs:2444` (the frame-pin no longer preempts
/// chosen_action when softmax picked Caretake), the yield rule is
/// structurally unnecessary — Caretake's high L2 score in the acute
/// case wins softmax naturally; the pin's Caretake-preempts guard
/// preserves the rescue path; the held rear_kitten frame stays on
/// the stack as durable commitment per §8.4. When Caretake's score
/// drops (kitten sated after feeding), the next-tick softmax picks
/// another DSE, the pin guard does not fire, and the pin resumes
/// walking sub-goals. Both effects (the previous yield-rule
/// suppression and the new pin-guard preservation) compose the same
/// rescue path; the pin-guard is more surgical (frame remains held
/// vs. being dropped by the rebuild on the yield path).
///
/// `HasJuvenileDependent` and `Parent` are authored by
/// [`crate::systems::growth::update_parent_markers`] in Chain 2a, in
/// one merged pass.
pub fn has_dependent_kitten(world: &World, entity: Entity) -> bool {
    let ent = world.entity(entity);
    !ent.contains::<Dead>() && ent.contains::<Parent>() && ent.contains::<HasJuvenileDependent>()
}

/// Construct the `rear_kitten` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn rear_kitten() -> Method {
    Method {
        id: MethodId("rear_kitten"),
        goal_label: "kitten_reared",
        applicable_when: ApplicableWhen::Live(has_dependent_kitten),
        sub_goals: &[
            SubGoal::Primitive {
                label: "wean_kitten",
                action: Action::Wean,
                target_hint: TargetHint::DependentKitten,
            },
            SubGoal::Primitive {
                label: "teach_kitten",
                action: Action::Teach,
                target_hint: TargetHint::DependentKitten,
            },
            SubGoal::Primitive {
                label: "release_kitten",
                action: Action::Release,
                target_hint: TargetHint::DependentKitten,
            },
        ],
        // Backtrack: if the kitten Entity despawns or `KittenDependency`
        // disappears mid-arc (death, premature maturity, adoption
        // transfer), the parent goal walks the abandon path rather
        // than panicking. No sibling methods share `goal_label:
        // "kitten_reared"` today; backtrack effectively means abandon
        // until a sibling method is authored.
        failure_strategy: MethodFailure::Backtrack,
        // Rearing is reactive substrate (driven by KittenDependency,
        // not aspirational achievement). No `AspirationDomain` matches
        // it; the picker's domain-affinity fallback (§H step 3) is not
        // the emission path. Emission is part of the dispatch-wiring
        // follow-on.
        domain: None,
    }
}
