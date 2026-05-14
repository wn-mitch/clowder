//! `mourn_at_grave` — dormant HTN method.
//!
//! Multi-tick mourning arc decomposed into vigil-at-grave →
//! grief-in-den. Dormant pending #332 (grief-vigil action vocabulary),
//! which authors the Grave-target picker, the per-cat `Mourning`
//! Component (or equivalent grief-tracking substrate), and the
//! release sub-goal that terminates the arc. When #332 lands it
//! flips `applicable_when` to `ApplicableWhen::Live`, replaces the
//! placeholder `eventual` closure with a real grave-proximity +
//! mourning-active predicate, and appends the `release` sub-goal.
//!
//! Wires-method back-reference: `docs/open-work/tickets/332-grief-
//! vigil-action-vocabulary.md` carries `wires-method:
//! [mourn_at_grave]` in its frontmatter — verified by
//! `scripts/check_method_registry.sh` Pass B.
//!
//! ## TargetHint placeholder
//!
//! `src/ai/methods/mod.rs::TargetHint` declares only `Partner` today
//! (per the §6.3 target-taking DSE doctrine — don't pre-populate
//! speculative variants). The two Primitive sub-goals here use
//! `Partner` as a placeholder; #332 extends the enum with the real
//! `GraveTarget` variant at the same time it flips this method to
//! Live, and the placeholder gets replaced in the same commit.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;

/// Construct the dormant `mourn_at_grave` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`. The
/// `Method` literal sits in this single function so
/// `scripts/check_method_registry.sh` can extract `(method_id,
/// blocker)` via its `Method { … }` block walker.
pub fn mourn_at_grave() -> Method {
    Method {
        id: MethodId("mourn_at_grave"),
        goal_label: "process_grief",
        applicable_when: ApplicableWhen::PendingSubstrate {
            blocker: "332",
            // Placeholder. `MethodRegistry::lookup` filters out
            // PendingSubstrate methods unconditionally, so this is
            // never invoked while the variant is dormant. #332
            // replaces it with the real (mourning-active &&
            // grave-in-range) check.
            eventual: |_world, _entity| false,
        },
        sub_goals: &[
            SubGoal::Primitive {
                label: "vigil_at_grave",
                action: Action::Vigil,
                target_hint: TargetHint::Partner,
            },
            SubGoal::Primitive {
                label: "grieve_in_den",
                action: Action::GriefSit,
                target_hint: TargetHint::Partner,
            },
            // The terminal `release` sub-goal lands with #332.
        ],
        failure_strategy: MethodFailure::Backtrack,
    }
}
