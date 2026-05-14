//! `acquire_stealth_via_self_craft` + `acquire_stealth_via_commission`
//! — dormant HTN methods for the worked example in
//! `docs/systems/htn-methods.md` §Worked example.
//!
//! Two backtracking siblings sharing the goal label
//! `stealth_gear_acquired`. Self-craft is the "I do it myself" path
//! (gather materials → workshop → craft → don). Commission is the
//! "I get someone else to do it" path (petition coordinator → wait →
//! retrieve → don). The L2 evaluator picks whichever method's
//! `eventual` predicate holds (e.g., self-craft requires the cat
//! has CraftingAffinity, commission requires a coordinator in
//! range); if neither holds, `MethodRegistry::lookup` falls through
//! to the 126 direct-adoption path.
//!
//! Both methods are dormant pending #334 (stealth-cloak crafting
//! recipe + WearItem resolver), which ships:
//! - The StealthCloak recipe in the crafting substrate.
//! - The real `WearItem` step resolver.
//! - The slot-inventory Component + writer.
//! - Hunt-resolver wearable-effect read (stalk-success multiplier).
//! - Possibly external prerequisites: generic crafting substrate +
//!   slot-inventory substrate.
//!
//! Wires-method back-reference: `docs/open-work/tickets/334-stealth-
//! cloak-crafting-recipe-wearitem-resolver.md` carries
//! `wires-method: [acquire_stealth_via_self_craft,
//! acquire_stealth_via_commission]` in its frontmatter — verified by
//! `scripts/check_method_registry.sh` Pass B.
//!
//! ## TargetHint placeholder
//!
//! `src/ai/methods/mod.rs::TargetHint` declares only `Partner` today
//! (per the §6.3 target-taking DSE doctrine). The Primitive sub-goals
//! here use `Partner` as a placeholder; #334 extends the enum with
//! the real target variants (`WorkshopTarget`, `CoordinatorTarget`,
//! `WornGearTarget`, …) at the same time it flips both methods to
//! Live, and the placeholders get replaced in the same commit.

use crate::ai::methods::{
    ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint,
};
use crate::ai::Action;

/// Construct the dormant `acquire_stealth_via_self_craft` method
/// literal. Called by `populate_method_registry` in
/// `src/plugins/simulation.rs`.
///
/// The full sub-goal chain (gather materials → reach workshop →
/// craft → don) lands with #334; this dormant shape carries only
/// the leaf primitives (`Craft` then `WearItem`) and omits the
/// compound `Goal("stealth_materials_in_inventory")` recursion +
/// the workshop-reach navigation primitive, both of which need
/// substrate (`StatePredicate` extensions, navigation primitives)
/// that #334 introduces. PendingSubstrate methods exist for type-
/// checking the leaf shape, not to encode the final recursion.
pub fn acquire_stealth_via_self_craft() -> Method {
    Method {
        id: MethodId("acquire_stealth_via_self_craft"),
        goal_label: "stealth_gear_acquired",
        applicable_when: ApplicableWhen::PendingSubstrate {
            blocker: "334",
            // Placeholder. #334 replaces it with the real
            // "has crafting affinity && materials available" check.
            eventual: |_world, _entity| false,
        },
        sub_goals: &[
            SubGoal::Primitive {
                label: "craft_stealth_cloak",
                action: Action::Craft,
                target_hint: TargetHint::Partner,
            },
            SubGoal::Primitive {
                label: "don_gear",
                action: Action::WearItem,
                target_hint: TargetHint::Partner,
            },
        ],
        failure_strategy: MethodFailure::Backtrack,
        domain: None,
    }
}

/// Construct the dormant `acquire_stealth_via_commission` method
/// literal. Sibling of `acquire_stealth_via_self_craft` under the
/// same `stealth_gear_acquired` goal label — the registry's first-
/// applicable scan picks whichever predicate holds; backtrack-on-
/// failure walks to the sibling.
pub fn acquire_stealth_via_commission() -> Method {
    Method {
        id: MethodId("acquire_stealth_via_commission"),
        goal_label: "stealth_gear_acquired",
        applicable_when: ApplicableWhen::PendingSubstrate {
            blocker: "334",
            // Placeholder. #334 replaces it with the real
            // "coordinator-in-range && willing-to-commission" check.
            eventual: |_world, _entity| false,
        },
        sub_goals: &[
            SubGoal::Primitive {
                label: "petition_for_gear",
                action: Action::PetitionCoordinator,
                target_hint: TargetHint::Partner,
            },
            SubGoal::Primitive {
                label: "don_gear",
                action: Action::WearItem,
                target_hint: TargetHint::Partner,
            },
            // The full chain (petition → await → retrieve → don)
            // lands with #334; this dormant shape carries only the
            // entry-and-exit primitives so the type-check exercises
            // both new Action variants.
        ],
        failure_strategy: MethodFailure::Backtrack,
        domain: None,
    }
}
