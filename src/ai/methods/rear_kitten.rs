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
use crate::components::physical::Dead;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate — the cat is alive (placeholder).
///
/// Read by `MethodRegistry::lookup`. The intended precise predicate
/// is "any Entity in the world carries
/// `KittenDependency.mother == Some(self)`", but that requires
/// enumerating archetypes, and the HTN `applicable_when` signature
/// (`fn(&World, Entity) -> bool`) only exposes per-entity component
/// access via `world.entity(entity)` — Bevy 0.18 reserves world-wide
/// query iteration for `&mut World` paths.
///
/// Two options were considered:
/// (a) maintain a reverse-lookup `Resource` (e.g.
///     `KittenMotherIndex: HashMap<Entity, Vec<Entity>>`) updated by
///     an exclusive system,
/// (b) attach a `MotherOfDependent` marker Component to mothers and
///     remove it when their last kitten matures.
///
/// Both are real substrate that earns its keep at dispatch time
/// (the dispatch resolver also needs to look up the kitten target).
/// Authoring either at #333's scope is premature — the dispatch
/// follow-on (named in #333's landing Log) authors the same
/// reverse-lookup once for both `applicable_when` and the
/// kitten-target picker. Until then, the method is Live in the
/// registry but never selected (no aspiration emits `kitten_reared`
/// either, so the picker's emit-walk doesn't reach the registry
/// lookup), so the permissive `is alive` placeholder is honest:
/// the method *will* apply to any alive cat once dispatch lands;
/// the precise gate moves to the resolver until then.
fn cat_is_alive(world: &World, entity: Entity) -> bool {
    !world.entity(entity).contains::<Dead>()
}

/// Construct the `rear_kitten` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn rear_kitten() -> Method {
    Method {
        id: MethodId("rear_kitten"),
        goal_label: "kitten_reared",
        applicable_when: ApplicableWhen::Live(cat_is_alive),
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
