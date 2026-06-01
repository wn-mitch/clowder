//! `CraftAtWorkshop` — ticket 457 / parameterized in 463 commit 8.
//!
//! Workshop-craft resolver. Takes a `RecipeId` from the plan step
//! (the recipe identity flows from the held `Intention::Goal(HaveItem(_))`
//! through `craft_have_item_actions`'s plan template). Looks the
//! recipe up in the registry, drains its inputs from the actor's
//! `Inventory`, spawns the output per `Recipe.output.destination`, and
//! emits `Feature::ItemCrafted`. Retired the pre-463 lex-pick — the
//! resolver no longer chooses "best satisfied" recipe; the choice
//! happens upstream in the aspiration picker.

use crate::components::equipment::WearableSlots;
use crate::components::items::ItemModifiers;
use crate::components::magic::{Inventory, ItemSlot};
use crate::components::physical::Position;
use crate::components::recipe::{ItemDestination, RecipeId, StationRequirement};
use crate::resources::recipe_registry::RecipeRegistry;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `CraftAtWorkshop`
///
/// **Real-world effect** — consumes one Workshop recipe's full input
/// set from the actor's `Inventory` pouch and produces the output:
/// `ItemDestination::Inventory` outputs land in the pouch;
/// `ItemDestination::EquippedSlot` outputs auto-equip into the cat's
/// `WearableSlots` (017), falling back to the pouch if the slot is
/// occupied. Recipe identity is pinned upstream — no resolver-side
/// selection.
///
/// **Plan-level preconditions** — emitted under `StatePredicate::ZoneIs(
/// PlannerZone::Workshop)` by `craft_have_item_actions` in
/// `src/ai/planner/actions.rs`. Cat eligibility + station availability
/// are gated upstream at `CraftAtWorkshopDse` (`CanCraft` +
/// `HasFunctionalWorkshop` + `CanSatisfyAnyWorkshopRecipeFromPouch` +
/// forbid `Incapacitated`).
///
/// **Runtime preconditions** — re-checks that a Workshop exists within
/// `proximity` tiles AND that the actor's inventory satisfies *the
/// named recipe* in full. The pouch may have lost an input between
/// `RetrieveCraftInputs` and `CraftAt<Station>` (dropped en route, or
/// consumed by a sibling step). On drift, returns `unwitnessed(Fail)`
/// — the planner re-picks.
///
/// **Witness** — `StepOutcome<RecipeId>`. Carries the parameterized
/// `recipe_id` on success.
///
/// **Feature emission** — caller passes `Feature::ItemCrafted`
/// (Positive, expected_to_fire_per_soak = true — the first-light gate
/// for the 368 Phase 2 behavioral tools) to `record_if_witnessed`.
pub fn resolve_craft_at_workshop(
    recipe_id: RecipeId,
    cat_pos: Position,
    inventory: &mut Inventory,
    wearables: &mut WearableSlots,
    recipes: &RecipeRegistry,
    workshop_positions: &[Position],
    proximity: f32,
) -> StepOutcome<Option<RecipeId>> {
    resolve_craft_at_station(
        recipe_id,
        cat_pos,
        inventory,
        wearables,
        recipes,
        workshop_positions,
        StationRequirement::Workshop,
        "workshop",
        proximity,
    )
}

/// 369 / 463 commit 8: shared station-craft resolver. Takes a
/// `RecipeId` from the plan step (the recipe identity flows from the
/// held HaveItem Intention through `craft_have_item_actions`'s
/// templated plan). Verifies station proximity + the named recipe's
/// station + the named recipe's full input set, drains, spawns,
/// witnesses. Used by both `resolve_craft_at_workshop` (Workshop) and
/// `resolve_craft_at_tanning_frame` (TanningFrame). The
/// `station_label` is for failure-mode strings; the `station_filter`
/// is a defensive check that the named recipe's station matches the
/// arm — a registration mistake (e.g. naming a Kitchen recipe on the
/// Workshop arm) returns `Fail`.
///
/// 468 retired the legacy `Option<RecipeId>` shape and the matching
/// `pick_satisfied_recipe` lex-pick fallback. Every CraftAt step
/// entering this resolver carries a specific RecipeId pinned upstream
/// by the HaveItem-aspiration template (`craft_have_item_actions`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_craft_at_station(
    recipe_id: RecipeId,
    cat_pos: Position,
    inventory: &mut Inventory,
    wearables: &mut WearableSlots,
    recipes: &RecipeRegistry,
    station_positions: &[Position],
    station_filter: StationRequirement,
    station_label: &'static str,
    proximity: f32,
) -> StepOutcome<Option<RecipeId>> {
    let near_station = station_positions
        .iter()
        .any(|sp| cat_pos.distance_to(sp) <= proximity);
    if !near_station {
        return StepOutcome::unwitnessed(StepResult::Fail(format!("no {station_label} in range")));
    }

    let Some(recipe) = recipes.get(recipe_id).cloned() else {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "{station_label}: recipe {} not in registry",
            recipe_id.0
        )));
    };
    if recipe.station != station_filter {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "{station_label}: recipe {} targets a different station ({:?})",
            recipe_id.0, recipe.station
        )));
    }
    // Defensive re-check — `RetrieveCraftInputs` upstream guarantees
    // the pouch carries the recipe's inputs at plan-emit time, but
    // inputs may have been dropped or destroyed en route. On miss,
    // re-plan rather than partially-craft.
    if !inventory.satisfies_recipe(&recipe) {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "{station_label}: recipe {} inputs not satisfied by inventory",
            recipe_id.0
        )));
    }
    let chosen_id = recipe_id;

    for input in &recipe.inputs {
        for _ in 0..input.count {
            let idx = inventory
                .pouch
                .iter()
                .position(|s| s.kind == input.kind)
                .expect("input verified present by Inventory::satisfies_recipe");
            inventory.pouch.swap_remove(idx);
        }
    }

    match recipe.output.destination {
        ItemDestination::Inventory => {
            if !inventory.add_item(recipe.output.item_kind) {
                return StepOutcome::unwitnessed(StepResult::Fail(
                    "inventory full at output add (shouldn't happen post-consume)".into(),
                ));
            }
        }
        ItemDestination::EquippedSlot => {
            // 017 — auto-equip the freshly-crafted wearable into its
            // anatomical slot. If the slot is already occupied (or the
            // kind isn't equippable), the new item stays in the pouch as
            // carried/unworn gear — crafting never displaces worn gear
            // (deliberate don/doff/swap is ticket 334). Either way the
            // item exists, so the craft is witnessed.
            let kind = recipe.output.item_kind;
            let item = ItemSlot::new(kind, ItemModifiers::default());
            let slot_free = kind
                .equip_slot()
                .is_some_and(|s| wearables.get(s).is_none());
            if slot_free {
                let _ = wearables.equip(item);
            } else if !inventory.add_item(kind) {
                return StepOutcome::unwitnessed(StepResult::Fail(
                    "pouch full at equipped-output fallback".into(),
                ));
            }
        }
        ItemDestination::WorldPosition => {
            return StepOutcome::unwitnessed(StepResult::Fail(format!(
                "{station_label} recipe output destination WorldPosition not yet \
                 supported (place-anchored decorations land in Phase 4)"
            )));
        }
    }

    StepOutcome::witnessed_with(StepResult::Advance, chosen_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::equipment::{EquipSlot, WearableSlots};
    use crate::components::items::{ItemKind, ItemModifiers};
    use crate::components::recipe::DisciplineKind;
    use crate::components::recipe::{Recipe, RecipeDuration, RecipeInput, RecipeOutput};

    /// A Workshop recipe producing a wielded weapon, routed to the equip
    /// slot (017). Bone input → BoneTipSpear, EquippedSlot destination.
    fn spear_recipe() -> Recipe {
        Recipe {
            id: RecipeId("test_spear"),
            discipline: DisciplineKind::BoneShellCraft,
            inputs: vec![RecipeInput {
                kind: ItemKind::Bone,
                count: 1,
            }],
            station: StationRequirement::Workshop,
            duration: RecipeDuration::Fixed { ticks: 1 },
            output: RecipeOutput {
                item_kind: ItemKind::BoneTipSpear,
                destination: ItemDestination::EquippedSlot,
            },
            skill_gate: None,
            is_warriors_kit: true,
            discipline_skill_affinity: None,
        }
    }

    fn registry_with(recipe: Recipe) -> RecipeRegistry {
        let mut r = RecipeRegistry::default();
        r.insert(recipe);
        r
    }

    #[test]
    fn equipped_slot_output_auto_equips_into_worn_slot() {
        let mut inv = Inventory::default();
        inv.add_item(ItemKind::Bone);
        let mut wearables = WearableSlots::default();
        let stations = [Position::new(0, 0)];

        let outcome = resolve_craft_at_workshop(
            RecipeId("test_spear"),
            Position::new(0, 0),
            &mut inv,
            &mut wearables,
            &registry_with(spear_recipe()),
            &stations,
            3.0,
        );

        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness.is_some(), "craft witnessed");
        // The spear is worn, not in the pouch; the Bone input was consumed.
        assert_eq!(
            wearables.get(EquipSlot::Wielded).map(|s| s.kind),
            Some(ItemKind::BoneTipSpear)
        );
        assert!(inv.pouch.is_empty(), "Bone consumed, spear not in pouch");
    }

    #[test]
    fn equipped_slot_falls_back_to_pouch_when_slot_occupied() {
        let mut inv = Inventory::default();
        inv.add_item(ItemKind::Bone);
        // Already wielding a flint blade — the Wielded slot is taken.
        let mut wearables = WearableSlots::default();
        wearables
            .equip(ItemSlot::new(
                ItemKind::FlintBlade,
                ItemModifiers::default(),
            ))
            .unwrap();
        let stations = [Position::new(0, 0)];

        let outcome = resolve_craft_at_workshop(
            RecipeId("test_spear"),
            Position::new(0, 0),
            &mut inv,
            &mut wearables,
            &registry_with(spear_recipe()),
            &stations,
            3.0,
        );

        assert!(matches!(outcome.result, StepResult::Advance));
        // Crafting never displaces worn gear (deliberate swap is 334) — the
        // flint blade stays wielded and the new spear lands in the pouch.
        assert_eq!(
            wearables.get(EquipSlot::Wielded).map(|s| s.kind),
            Some(ItemKind::FlintBlade)
        );
        assert!(inv.pouch.iter().any(|s| s.kind == ItemKind::BoneTipSpear));
    }
}
