//! `rear_kitten` — dormant HTN method.
//!
//! Multi-stage kitten-rearing arc decomposed into wean → teach →
//! release, keyed to a specific kitten Entity per method frame
//! (one frame per kitten the mother is rearing — sibling kittens get
//! sibling frames, not a single shared frame). Dormant pending #333
//! (kitten-rearing action vocabulary), which authors the kitten-
//! target picker, extends or pairs with `KittenDependency` to
//! advance through wean/teach/release milestones, and decides
//! whether a fresh `RearKittenIntent` Component or an extension of
//! the existing Caretake DSE is the right substrate seat.
//!
//! Wires-method back-reference: `docs/open-work/tickets/333-kitten-
//! rearing-action-vocabulary.md` carries `wires-method:
//! [rear_kitten]` in its frontmatter — verified by
//! `scripts/check_method_registry.sh` Pass B.
//!
//! ## TargetHint placeholder
//!
//! `src/ai/methods/mod.rs::TargetHint` declares only `Partner` today
//! (per the §6.3 target-taking DSE doctrine). The three Primitive
//! sub-goals here use `Partner` as a placeholder; #333 extends the
//! enum with the real `KittenTarget` variant at the same time it
//! flips this method to Live, and the placeholder gets replaced in
//! the same commit.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;

/// Construct the dormant `rear_kitten` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
pub fn rear_kitten() -> Method {
    Method {
        id: MethodId("rear_kitten"),
        goal_label: "kitten_reared",
        applicable_when: ApplicableWhen::PendingSubstrate {
            blocker: "357",
            // Placeholder. #357 dispatches the rear_kitten action
            // vocabulary substrate landed by #333; the real
            // "mother && has-dependent-kitten" check lands there.
            eventual: |_world, _entity| false,
        },
        sub_goals: &[
            SubGoal::Primitive {
                label: "wean_kitten",
                action: Action::Wean,
                target_hint: TargetHint::Partner,
            },
            SubGoal::Primitive {
                label: "teach_kitten",
                action: Action::Teach,
                target_hint: TargetHint::Partner,
            },
            SubGoal::Primitive {
                label: "release_kitten",
                action: Action::Release,
                target_hint: TargetHint::Partner,
            },
        ],
        // Backtrack: if the kitten Entity despawns or `KittenDependency`
        // disappears mid-arc, the parent goal falls back to sibling
        // methods (e.g., a future `mourn_at_grave` if the kitten
        // died) rather than abandoning silently.
        failure_strategy: MethodFailure::Backtrack,
        domain: None,
    }
}
