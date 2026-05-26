//! `seek_healing` — dormant HTN method for the festering-wound aftermath
//! arc (ticket 472).
//!
//! Decomposes the compound goal `festering_wound_healed` into rest +
//! accept-tending sub-goals. **Dormant** on `ApplicableWhen::
//! PendingSubstrate { blocker: "473" }` until ticket 473 lands the
//! `TendFestering` cat-side DSE that authors the corrupted-kin
//! perception map and lifts the recipient-side completion proxy. The
//! method exists at land for two reasons:
//!
//! 1. **Type-system anchor.** Registering it under
//!    `populate_method_registry` exercises the goal-label →
//!    decomposition typecheck and proves 472's `has_festering_wound`
//!    predicate compiles.
//! 2. **Glue-ticket discipline.** Per CLAUDE.md "Every dormant method
//!    has a glue ticket," the `blocker: "473"` here must point at an
//!    open ticket whose frontmatter carries
//!    `wires-method: [seek_healing]`. 473's open file gets that
//!    frontmatter in this same commit; `scripts/check_method_registry.sh`
//!    Pass B verifies both directions.
//!
//! ## TargetHint placeholder
//!
//! Today's `TargetHint` enum declares only `Partner`. The sub-goals
//! below use `Partner` as a placeholder; 473 extends the enum with
//! the recipient-side variants (`KinHealerTarget`, …) at the same
//! time it flips this method to Live.

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;

/// Construct the dormant `seek_healing` method literal. Called by
/// `populate_method_registry` in `src/plugins/simulation.rs`.
///
/// `eventual` predicate compiles against 472's `has_festering_wound`
/// helper (see `src/components/body_zones.rs`). The L2 evaluator never
/// invokes it while the method is `PendingSubstrate`, but the body
/// model type-checks the predicate signature.
pub fn seek_healing() -> Method {
    Method {
        id: MethodId("seek_healing"),
        goal_label: "festering_wound_healed",
        applicable_when: ApplicableWhen::PendingSubstrate {
            blocker: "473",
            eventual: |world, entity| {
                world
                    .get::<crate::components::body_zones::CatBodyModel>(entity)
                    .map(|m| m.has_festering_wound())
                    .unwrap_or(false)
            },
        },
        sub_goals: &[
            // Rest at the lair while the wound recovers under slow
            // passive heal + active intervention. 473 wires the
            // real predicates that gate the chain on a kin healer
            // being in range.
            SubGoal::Primitive {
                label: "rest_with_wound",
                action: Action::Sleep,
                target_hint: TargetHint::Partner,
            },
            // Accept tending from a peer carrying out `TendFestering`
            // (473). Placeholder leaf primitive — 473 introduces the
            // recipient-side action variant; today this stands in.
            SubGoal::Primitive {
                label: "accept_tending",
                action: Action::Socialize,
                target_hint: TargetHint::Partner,
            },
        ],
        failure_strategy: MethodFailure::Backtrack,
        domain: None,
    }
}
