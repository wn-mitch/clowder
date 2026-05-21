//! `mourn_at_grave` — Live HTN method (#332).
//!
//! Multi-tick mourning arc decomposed into vigil-at-grave →
//! grieve-in-den → release-grief, keyed to the cat's
//! [`Mourning`](crate::components::Mourning) Component. Authored
//! at #332 alongside the action vocabulary (`Action::Vigil`,
//! `Action::GriefSit`, `Action::ReleaseGrief`), the substrate
//! Component, and the [`TargetHint::Grave`](super::TargetHint) bind.
//!
//! # Status
//!
//! `applicable_when: Live` — the method registers Live in
//! `MethodRegistry`. Its `applicable_when` predicate gates on
//! `Mourning` presence, so the method is selectable only for cats
//! actively mourning a colony-mate.
//!
//! **Dispatch wiring is pending** — the cat's `chosen_action` is
//! still picked by the per-tick DSE softmax, not by the HTN method's
//! primitive sub-goals. The follow-on dispatch ticket (named in
//! #332's landing Log) wires DSE / GoapActionKind / plan template /
//! resolver call site so the cat's behavior actually advances the
//! method's sub-goals. The Mourning *insertion* path (when a
//! colony-mate dies) is the §7.7.b grief-event-emission debt
//! tracked under 060 Phase 6b — also out of scope for #332.
//!
//! # Why `applicable_when` checks `Mourning` (not just `Has<Grave>`)
//!
//! `Has<Grave>` would return true for any cat near *any* grave —
//! the method would apply colony-wide whenever a grave exists. The
//! `Mourning` Component carries the cat's specific grief commitment
//! (`deceased_name`); the method applies only while the commitment
//! is held, not as a passive response to any grave's existence. The
//! grave-target picker (`pick_grave_for_mourner`) reads
//! `Mourning.deceased_name` to find the *correct* grave.

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::mourning::Mourning;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate — the cat holds an active `Mourning`
/// Component. Read by `MethodRegistry::lookup`.
fn has_active_mourning(world: &World, entity: Entity) -> bool {
    world.entity(entity).contains::<Mourning>()
}

/// Construct the `mourn_at_grave` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn mourn_at_grave() -> Method {
    Method {
        id: MethodId("mourn_at_grave"),
        goal_label: "process_grief",
        applicable_when: ApplicableWhen::Live(has_active_mourning),
        sub_goals: &[
            SubGoal::Primitive {
                label: "vigil_at_grave",
                action: Action::Vigil,
                target_hint: TargetHint::Grave,
            },
            SubGoal::Primitive {
                label: "grieve_in_den",
                action: Action::GriefSit,
                target_hint: TargetHint::Grave,
            },
            SubGoal::Primitive {
                label: "release_grief",
                action: Action::ReleaseGrief,
                target_hint: TargetHint::Grave,
            },
        ],
        // Backtrack: if `Mourning` is removed mid-arc (whether by the
        // terminal `release_grief` sub-goal or by an external system),
        // the parent goal walks the abandon path rather than panicking.
        // No sibling methods share `goal_label: "process_grief"` today,
        // so backtrack effectively means "abandon" until a sibling
        // method is authored — which is the right shape; abandonment
        // here is correct semantics, not a defect.
        failure_strategy: MethodFailure::Backtrack,
        // Mourning is reactive substrate, not aspirational achievement;
        // no `AspirationDomain` matches it. The picker's domain-affinity
        // fallback (§H step 3) is therefore not the emission path —
        // emission for reactive substrate is part of the dispatch-
        // wiring follow-on.
        domain: None,
    }
}
