//! `acquire_stealth_via_self_craft` + `acquire_stealth_via_commission`
//! — HTN methods for the worked example in
//! `docs/systems/htn-methods.md` §Worked example. Self-craft is Live as
//! of #334; commission stays dormant pending #481 / #381.
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
//! `acquire_stealth_via_self_craft` is **Live** as of #334, which shipped:
//! - The real `WearItem` step resolver (`src/steps/disposition/wear_item.rs`).
//! - `Action::WearItem` HTN-primitive wiring (planner + dispatch + frame
//!   advance).
//! - The Craft frame-pin seam: the `craft_stealth_cloak` leaf routes through
//!   the 463 HaveItem craft template (`craft_have_item_actions`) with the
//!   woven reed cloak's recipe pinned via `TargetHint::CraftItem`.
//!
//! The stealth-cloak item-of-record (`ItemKind::WovenReedCloak` + recipe
//! `warriors_kit.woven_reed_cloak`) and the slot-inventory substrate
//! (`WearableSlots`) landed earlier in 369 and 017; the cloak's stealth
//! effect (detection-masking) is already read by `prey.rs::try_detect_cat`
//! via the 477 modifier-aggregation layer.
//!
//! `acquire_stealth_via_commission` stays **dormant** — its substrate does
//! not exist yet (`resolve_petition_coordinator` is a Fail-stub,
//! `Action::PetitionCoordinator` has no `htn_primitive_actions` arm, and
//! `Goal("ordered_item_ready")` needs the trader/coordinator-commission loop
//! parked under #381). Its `PendingSubstrate` blocker re-points from "334"
//! to #481, the glue ticket that holds the commission wiring; #481's
//! frontmatter carries `wires-method: [acquire_stealth_via_commission]`
//! (verified by `scripts/check_method_registry.sh` Pass B).

use crate::ai::methods::{ApplicableWhen, Method, MethodFailure, MethodId, SubGoal, TargetHint};
use crate::ai::Action;
use crate::components::equipment::{EquipSlot, WearableSlots};
use crate::components::items::ItemKind;
use crate::components::markers;
use bevy_ecs::prelude::*;

/// `applicable_when` predicate for `acquire_stealth_via_self_craft` — the cat
/// can craft (Adult ∧ ¬Injured, surfaced by the `CanCraft` marker Component)
/// AND lacks a cloak in its Cape slot. A cat that already wears a cloak has
/// no reason to acquire one; a cat that can't craft falls through to the
/// dormant commission sibling (and, today, to the 126 direct-adoption path).
fn can_self_craft_stealth(world: &World, entity: Entity) -> bool {
    let cat = world.entity(entity);
    let lacks_cloak = cat
        .get::<WearableSlots>()
        .is_none_or(|slots| slots.get(EquipSlot::Cape).is_none());
    let can_craft = cat.contains::<markers::CanCraft>();
    lacks_cloak && can_craft
}

/// Construct the Live `acquire_stealth_via_self_craft` method literal.
/// Called by `populate_method_registry` in `src/plugins/simulation.rs`.
///
/// Two-leg chain: the `craft_stealth_cloak` leaf reuses the 463 HaveItem
/// craft template (gather inputs → travel to Workshop → craft) with the
/// cloak's recipe pinned by `TargetHint::CraftItem(WovenReedCloak)`; the
/// `don_gear` leaf dons it via the `WearItem` resolver. In the happy path
/// the cloak auto-equips on craft (017), so the don leaf is an idempotent
/// success — it becomes load-bearing only when the Cape slot was occupied
/// at craft time (a swap). The gather → workshop legs are synthesized inside
/// `craft_have_item_actions`, so no compound materials Goal is needed here.
pub fn acquire_stealth_via_self_craft() -> Method {
    Method {
        id: MethodId("acquire_stealth_via_self_craft"),
        goal_label: "stealth_gear_acquired",
        applicable_when: ApplicableWhen::Live(can_self_craft_stealth),
        sub_goals: &[
            SubGoal::Primitive {
                label: "craft_stealth_cloak",
                action: Action::Craft,
                target_hint: TargetHint::CraftItem(ItemKind::WovenReedCloak),
            },
            SubGoal::Primitive {
                label: "don_gear",
                action: Action::WearItem,
                target_hint: TargetHint::WornGear,
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
            blocker: "481",
            // Placeholder. #481 replaces it with the real
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
                target_hint: TargetHint::WornGear,
            },
            // The full chain (petition → await → retrieve → don)
            // lands with #481 (blocked on the #381 trader/coordinator-
            // commission substrate); this dormant shape carries only the
            // entry-and-exit primitives so the type-check exercises both
            // Action variants. `don_gear` already reuses 334's WearItem
            // resolver + `TargetHint::WornGear`.
        ],
        failure_strategy: MethodFailure::Backtrack,
        domain: None,
    }
}
