//! `caretake_kitten` — Live HTN method (ticket 398).
//!
//! Catches the `caretake_kitten` label that
//! [`crate::ai::aspirations::kinship::RAISE_OFFSPRING_ASPIRATION`]'s
//! single milestone emits when its emit row's `applicable_when`
//! predicate fires. At 398 Phase 1a the emit row is dormant
//! (`applicable_when: always_false`) so this method has no live
//! emission path yet — it registers as Live so the
//! `MethodRegistry::lookup(label, ...)` check in the picker's
//! `step2_emits_walk` resolves cleanly when the row activates in
//! 398's later phases.
//!
//! # Why this is a single-primitive method
//!
//! The §7.M.2 spec frames `RaiseOffspringAspiration` as emitting
//! Caretake Intentions per tick the parent has a juvenile dependent.
//! The Intention shape is a Goal — "feed and tend this kitten" — that
//! decomposes (HTN-style) into `Action::Caretake`, which itself
//! resolves to the existing 4-step plan template
//! (`[MoveTo(Stores) → RetrieveAnyFoodFromStores →
//! MoveTo(kitten) → FeedKitten]`) built by
//! `build_caretaking_chain` (`src/systems/disposition.rs:2827`).
//!
//! Wean / Teach / Release are not L3-selectable Actions
//! (`Action::Wean`/`Teach`/`Release` map to
//! `DispositionKind::from_action(_) => None`); they fire as
//! side-effects of the `FeedKitten` step succeeding in the right
//! maturity band (Phase 3f in the 398 plan). The HTN method
//! [`rear_kitten`](crate::ai::methods::rear_kitten) retains its
//! Wean → Teach → Release decomposition substrate, but the §7.4
//! commitment layer (which Intention is held) moves to the unified
//! softmax + persistence-bonus, not the frame-pin override.
//!
//! # `applicable_when` predicate
//!
//! Reuses
//! [`crate::ai::methods::rear_kitten::has_dependent_kitten`] — alive
//! cat carrying `Parent` AND `HasJuvenileDependent`. The two methods
//! share the same eligibility surface intentionally: §7.M.2's
//! Caretake-Intention emission is the per-tick "tend the dependent
//! kitten" semantic; `rear_kitten`'s Wean / Teach / Release is the
//! maturity-bump decomposition. They co-exist during the Phase 3
//! transition; 398's plan keeps `rear_kitten` registered as the
//! decomposition record while the held Intention slot belongs to
//! `caretake_kitten`.
//!
//! # `domain: Some(Kinship)`
//!
//! Exposes the method to the picker's §H step-3 domain-affinity
//! fallback for any Kinship-chain milestone whose `emits[]` table
//! is empty. At 398 Phase 1a `RAISE_OFFSPRING_ASPIRATION` has one
//! milestone with a non-empty (but dormant) `emits[]`, so the
//! fallback path doesn't fire on this chain — but exposing the
//! method by domain keeps the picker contract consistent and
//! supports the future case where additional Kinship chains land
//! (lifetime-celibacy or adoption arcs).

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Construct the `caretake_kitten` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn caretake_kitten() -> Method {
    Method {
        id: MethodId("caretake_kitten"),
        goal_label: "caretake_kitten",
        applicable_when: ApplicableWhen::Live(
            crate::ai::methods::rear_kitten::has_dependent_kitten,
        ),
        sub_goals: &[SubGoal::Primitive {
            label: "tend_dependent_kitten",
            action: Action::Caretake,
            target_hint: TargetHint::DependentKitten,
        }],
        // Abandon: there are no sibling methods for `caretake_kitten`.
        // If the leaf abandons (kitten despawn mid-arc, etc.), the
        // picker re-emits next tick via the aspiration's emit row when
        // the substrate is wired (398 Phase 1c+).
        failure_strategy: MethodFailure::Abandon,
        domain: Some(AspirationDomain::Kinship),
    }
}
