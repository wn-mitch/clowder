use crate::components::equipment::WearableSlots;
use crate::components::magic::Inventory;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `WearItem`
///
/// Don the first equippable wearable carried in the cat's pouch into its
/// anatomical [`WearableSlots`] — the `don_gear` leaf (sub-goal 1) of the
/// `acquire_stealth_via_self_craft` HTN method (ticket 334).
///
/// **Real-world effect** — moves the first pouch item whose kind maps to an
/// [`crate::components::equipment::EquipSlot`] out of `Inventory.pouch` and
/// into `WearableSlots`. If the destination slot was already occupied, the
/// displaced item is routed back to the pouch (a swap), preserving its
/// `ItemModifiers` and quality (the whole `ItemSlot` is pushed, not re-added
/// by kind). Once worn, the item's identity-keyed effects compose through
/// `equipment_modifiers_for` and are read by the relevant resolvers — e.g.
/// the woven reed cloak's `detection_visual_mask` + `noise_level` lower a
/// stalking cat's detection in `prey.rs::try_detect_cat`, per CLAUDE.md's
/// "items are real" pillar.
///
/// **Plan-level preconditions** — emitted with no preconditions by the
/// `htn_primitive_actions` builder (donning happens in place; there is no
/// travel and no zone gate). The held `acquire_stealth_via_self_craft` frame
/// pins this leaf after the `craft_stealth_cloak` leg.
///
/// **Runtime preconditions** — none. The pouch scan is total: when no pouch
/// item is equippable, the resolver returns `unwitnessed(Advance)`. This is
/// the dominant self-craft path — crafting a wearable auto-equips it on craft
/// (017) when the slot is free, so by the time the `don_gear` leaf runs the
/// cloak is usually already worn and the pouch holds no wearable. Returning
/// unwitnessed `Advance` (not witnessed) keeps the contract honest: the
/// gear-is-worn goal already holds, so advancing is correct, but no Feature
/// is recorded because no don occurred this call.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff an item was actually donned
/// or swapped into a slot this call; `false` on the idempotent no-op (nothing
/// equippable in the pouch) and on the unreachable not-equippable branch.
///
/// **Feature emission** — caller passes `Feature::ItemWorn` (Positive) to
/// `record_if_witnessed`. Fires only on a real don/swap; the idempotent
/// no-op records nothing (hence `expected_to_fire_per_soak() => false`).
pub fn resolve_wear_item(
    inventory: &mut Inventory,
    wearables: &mut WearableSlots,
) -> StepOutcome<bool> {
    let Some(idx) = inventory
        .pouch
        .iter()
        .position(|slot| slot.kind.equip_slot().is_some())
    else {
        // Nothing carried to don — the wearable was already auto-equipped
        // on craft (017). The gear-is-worn goal holds; advance without a
        // witness so no Feature is recorded for a no-op.
        return StepOutcome::<bool>::unwitnessed(StepResult::Advance);
    };

    let item = inventory.pouch.swap_remove(idx);
    match wearables.equip(item) {
        Ok(displaced) => {
            // Swap: route the previously-worn occupant back to the pouch,
            // preserving its identity/modifiers (push the slot, never
            // re-add by kind). Net-zero on pouch count, so this can't
            // overflow capacity.
            if let Some(prev) = displaced {
                inventory.pouch.push(prev);
            }
            StepOutcome::<bool>::witnessed(StepResult::Advance)
        }
        Err(returned) => {
            // Unreachable: `position` guaranteed `equip_slot().is_some()`.
            // Keep the item rather than drop it (items are real).
            inventory.pouch.push(returned);
            StepOutcome::<bool>::unwitnessed(StepResult::Advance)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::equipment::EquipSlot;
    use crate::components::items::{ItemKind, ItemModifiers};
    use crate::components::magic::ItemSlot;

    fn slot(kind: ItemKind) -> ItemSlot {
        ItemSlot::new(kind, ItemModifiers::default())
    }

    #[test]
    fn dons_carried_wearable_into_empty_slot() {
        let mut inv = Inventory::default();
        inv.pouch.push(slot(ItemKind::WovenReedCloak));
        let mut wearables = WearableSlots::default();

        let outcome = resolve_wear_item(&mut inv, &mut wearables);

        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness, "a real don is witnessed");
        assert_eq!(
            wearables.get(EquipSlot::Cape).map(|s| s.kind),
            Some(ItemKind::WovenReedCloak)
        );
        assert!(inv.pouch.is_empty(), "cloak left the pouch");
    }

    #[test]
    fn swap_routes_displaced_item_back_to_pouch_preserving_quality() {
        // A blade already wielded; a second wielded weapon in the pouch.
        let mut inv = Inventory::default();
        inv.pouch.push(ItemSlot::with_quality(
            ItemKind::BoneTipSpear,
            0.5,
            ItemModifiers::default(),
        ));
        let mut wearables = WearableSlots::default();
        wearables.equip(slot(ItemKind::FlintBlade)).unwrap();

        let outcome = resolve_wear_item(&mut inv, &mut wearables);

        assert!(outcome.witness, "a swap is witnessed");
        // The spear is now wielded; the displaced flint blade is back in
        // the pouch (slot preserved, not re-added by kind).
        assert_eq!(
            wearables.get(EquipSlot::Wielded).map(|s| s.kind),
            Some(ItemKind::BoneTipSpear)
        );
        assert_eq!(
            wearables.get(EquipSlot::Wielded).map(|s| s.quality),
            Some(0.5)
        );
        assert_eq!(inv.pouch.len(), 1, "displaced blade returned to pouch");
        assert_eq!(inv.pouch[0].kind, ItemKind::FlintBlade);
    }

    #[test]
    fn idempotent_no_op_when_nothing_equippable_in_pouch() {
        // The cloak is already worn (auto-equipped on craft); pouch holds
        // only a non-wearable. The don leaf must advance without witnessing.
        let mut inv = Inventory::default();
        inv.pouch.push(slot(ItemKind::RawMouse));
        let mut wearables = WearableSlots::default();
        wearables.equip(slot(ItemKind::WovenReedCloak)).unwrap();

        let outcome = resolve_wear_item(&mut inv, &mut wearables);

        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(!outcome.witness, "no don occurred → unwitnessed");
        assert_eq!(inv.pouch.len(), 1, "non-wearable left untouched");
        assert_eq!(
            wearables.get(EquipSlot::Cape).map(|s| s.kind),
            Some(ItemKind::WovenReedCloak)
        );
    }

    #[test]
    fn empty_pouch_advances_unwitnessed() {
        let mut inv = Inventory::default();
        let mut wearables = WearableSlots::default();

        let outcome = resolve_wear_item(&mut inv, &mut wearables);

        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(!outcome.witness);
    }
}
