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

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;
use crate::components::markers::Parent;
use crate::components::physical::Dead;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate — alive cat carrying the `Parent`
/// marker (ticket 357 refinement of the prior `cat_is_alive`
/// placeholder).
///
/// The `Parent` marker is authored by
/// [`crate::systems::growth::update_parent_markers`] in Chain 2a; its
/// predicate is "this cat has ≥1 living dependent kitten with
/// `KittenDependency.mother == self` OR `…father == self`." 357
/// reuses the existing reverse-lookup substrate rather than
/// introducing a sibling `MotherOfDependent` marker — the existing
/// single marker covers both parental roles, and the dependent-
/// kitten target picker filters mother-only (per #333 §Out of scope)
/// at target resolution time. If father involvement in rearing is
/// later authored as a separate aspiration (#333 §Out of scope), it
/// shares the same gate; only the picker's filter changes.
fn has_dependent_kitten(world: &World, entity: Entity) -> bool {
    let ent = world.entity(entity);
    !ent.contains::<Dead>() && ent.contains::<Parent>()
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
