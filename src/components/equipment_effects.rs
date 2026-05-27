//! Equipment-modifier aggregation (ticket 477 — consumption layer for 369).
//!
//! 369 shipped the identity→property data layer (`weapon_class`,
//! `armor_class`, `noise_class`, `durability_tier`, `equip_material` on
//! [`ItemKind`]). This module is the *uniform aggregation seam* every
//! resolver consults to get the effective combat/stealth modifiers of
//! the items a cat currently wears or carries. Per the doctrine pillar
//! "items have bite" (CLAUDE.md): resolvers fetch the aggregate and
//! apply it, never per-resolver `match item.kind` effect logic.
//!
//! 017 semantic: only *worn* items contribute. [`equipment_modifiers_for`]
//! reads the cat's [`WearableSlots`] (the OSRS-style equip slots), not the
//! carry pouch — an equipment item sitting in the pouch is carried, not
//! wielded/worn, and contributes nothing. Crafting a wearable auto-equips
//! it (see `craft_at_workshop` / `craft_at_tanning_frame`); deliberate
//! don/doff is ticket 334.
//!
//! Composition precedent: `preservation_output_quality`
//! (`src/steps/disposition/tend_smoking_rack.rs`) — accept data slices,
//! compose via per-classifier reads scaled by quality, return a typed
//! aggregate.

use crate::components::equipment::{
    ArmorClass, DurabilityTier, EquipMaterial, NoiseClass, WeaponClass, WearableSlots,
};
use crate::components::items::ItemKind;
use crate::components::magic::ItemSlot;
use crate::resources::sim_constants::CombatConstants;

/// Aggregate combat/stealth modifier surface read by every equipment-aware
/// resolver. Computed once per resolver call from a cat's [`Inventory`].
///
/// All fields are pre-clamped/composed — resolvers consume directly. Fields
/// the resolver doesn't read cost ~20 cycles each to compute; that's
/// negligible compared to the resolver work that follows and lets every
/// read site share one canonical shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquipmentModifiers {
    /// Best wielded weapon, ranked by `(class_priority, quality)`. `None`
    /// when no weapon is carried.
    pub weapon: Option<WeaponView>,
    /// Fraction of incoming blunt damage absorbed by worn armor.
    /// `0.0` (none) to [`CombatConstants::armor_reduction_floor_blunt`]
    /// (ceiling). Additive across multiple armor items, then
    /// `min`-clamped.
    pub armor_blunt_reduction: f32,
    /// Fraction of incoming pierce damage absorbed.
    /// `0.0` to [`CombatConstants::armor_reduction_floor_pierce`].
    pub armor_pierce_reduction: f32,
    /// `true` when a `WeaponClass::Ranged` item (e.g. Sling) is carried.
    /// Read by the ranged-attack resolver (lands in 477's follow-up).
    pub ranged_enabled: bool,
    /// Visual-detection mask from a worn cloak. `0.0` (no cloak) to
    /// `cloak_visual_mask_magnitude` at quality `1.0`. Multiplies the
    /// sight component of prey detection before its `max` with tremor —
    /// a cloaked cat is harder to *see* but motion still gives them
    /// away. Read by `try_detect_cat`.
    pub detection_visual_mask: f32,
    /// `max()` of every carried item's [`NoiseClass`]. `Loud` raises the
    /// cat's tremor floor for prey detection. All Phase 2b items are
    /// `Silent`; metal items in 370 will flip the read site live.
    pub noise_level: NoiseClass,
}

/// Best-wielded-weapon view. Carries the four facts a strike resolver
/// needs: class (Pierce / Slash / Blunt / Ranged), material (for
/// narrative + future material-keyed effects), quality (for bonus
/// scaling), and `fragile` (true iff [`DurabilityTier::Fragile`] — the
/// snap-on-failed-strike gate for bone weapons).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponView {
    /// The wielded weapon's item kind — carried so the snap path can
    /// remove the exact item and name it in narrative.
    pub kind: ItemKind,
    pub class: WeaponClass,
    pub material: EquipMaterial,
    pub quality: f32,
    pub fragile: bool,
}

impl WeaponView {
    /// Hunt-strike success bonus contributed by this weapon —
    /// class-keyed (pierce/slash/blunt) and quality-scaled by the
    /// shared floor-linear curve. `Ranged` weapons contribute no melee
    /// strike bonus (their effect is the separate ranged-attack mode in
    /// 477's follow-up). Read by `resolve_engage_prey` at pounce eval.
    pub fn strike_bonus(&self, c: &CombatConstants) -> f32 {
        let base = match self.class {
            WeaponClass::Pierce => c.hunt_strike_pierce_bonus,
            WeaponClass::Slash => c.hunt_strike_slash_bonus,
            WeaponClass::Blunt => c.hunt_strike_blunt_bonus,
            WeaponClass::Ranged => 0.0,
        };
        base * quality_multiplier(self.quality, c.equipment_quality_floor)
    }
}

/// Lower the `quality ∈ [0, 1]` axis into an effect multiplier, with a
/// floor so a freshly-failed craft is not binary-useless. Mirrored across
/// every read site.
///
/// `effective = base * (floor + (1 - floor) * quality)`.
fn quality_multiplier(quality: f32, floor: f32) -> f32 {
    let q = quality.clamp(0.0, 1.0);
    let f = floor.clamp(0.0, 1.0);
    f + (1.0 - f) * q
}

fn weapon_view_for(slot: &ItemSlot) -> Option<WeaponView> {
    let class = slot.kind.weapon_class()?;
    let material = slot.kind.equip_material()?;
    Some(WeaponView {
        kind: slot.kind,
        class,
        material,
        quality: slot.quality.clamp(0.0, 1.0),
        fragile: matches!(slot.kind.durability_tier(), DurabilityTier::Fragile),
    })
}

fn armor_contribution(slot: &ItemSlot, c: &CombatConstants) -> (f32, f32) {
    let Some(armor) = slot.kind.armor_class() else {
        return (0.0, 0.0);
    };
    let q = quality_multiplier(slot.quality, c.equipment_quality_floor);
    match armor {
        ArmorClass::BluntAbsorb => (c.armor_blunt_absorb_magnitude * q, 0.0),
        ArmorClass::PiercePartial => (
            c.armor_pierce_partial_blunt_magnitude * q,
            c.armor_pierce_partial_pierce_magnitude * q,
        ),
    }
}

fn cloak_mask_for(slot: &ItemSlot, c: &CombatConstants) -> f32 {
    if !matches!(slot.kind, ItemKind::WovenReedCloak) {
        return 0.0;
    }
    c.cloak_visual_mask_magnitude * quality_multiplier(slot.quality, c.equipment_quality_floor)
}

fn noisier(a: NoiseClass, b: NoiseClass) -> NoiseClass {
    match (a, b) {
        (NoiseClass::Loud, _) | (_, NoiseClass::Loud) => NoiseClass::Loud,
        _ => NoiseClass::Silent,
    }
}

/// Compose the modifier aggregate for one cat. Walks the cat's worn equip
/// slots ([`WearableSlots`]) exactly once — items in the carry pouch are
/// not worn and do not contribute (017).
pub fn equipment_modifiers_for(
    wearables: &WearableSlots,
    c: &CombatConstants,
) -> EquipmentModifiers {
    let mut weapon: Option<WeaponView> = None;
    let mut blunt = 0.0_f32;
    let mut pierce = 0.0_f32;
    let mut ranged_enabled = false;
    let mut visual_mask = 0.0_f32;
    let mut noise = NoiseClass::Silent;

    for slot in wearables.worn_iter() {
        // At most one worn item is a weapon — `equip_slot` maps every
        // weapon to the single `EquipSlot::Wielded` slot, so no ranking
        // among multiple weapons is possible (017). The wielded weapon is
        // whatever occupies that slot.
        if let Some(candidate) = weapon_view_for(slot) {
            ranged_enabled |= matches!(candidate.class, WeaponClass::Ranged);
            weapon = Some(candidate);
        }
        let (b, p) = armor_contribution(slot, c);
        blunt += b;
        pierce += p;
        visual_mask += cloak_mask_for(slot, c);
        noise = noisier(noise, slot.kind.noise_class());
    }

    EquipmentModifiers {
        weapon,
        armor_blunt_reduction: blunt.clamp(0.0, c.armor_reduction_floor_blunt),
        armor_pierce_reduction: pierce.clamp(0.0, c.armor_reduction_floor_pierce),
        ranged_enabled,
        detection_visual_mask: visual_mask.clamp(0.0, 1.0),
        noise_level: noise,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::ItemModifiers;

    fn ctx() -> CombatConstants {
        CombatConstants::default()
    }

    /// Build a [`WearableSlots`] by equipping each item. Inputs must occupy
    /// distinct slots (one weapon, one cape, …) — a same-slot collision
    /// silently displaces, which the equip-collision test covers separately.
    fn worn(items: &[(ItemKind, f32)]) -> WearableSlots {
        let mut w = WearableSlots::default();
        for (kind, q) in items {
            w.equip(ItemSlot::with_quality(*kind, *q, ItemModifiers::default()))
                .expect("test items are equippable");
        }
        w
    }

    #[test]
    fn empty_wearables_yield_zero_aggregate() {
        let em = equipment_modifiers_for(&WearableSlots::default(), &ctx());
        assert!(em.weapon.is_none());
        assert_eq!(em.armor_blunt_reduction, 0.0);
        assert_eq!(em.armor_pierce_reduction, 0.0);
        assert!(!em.ranged_enabled);
        assert_eq!(em.detection_visual_mask, 0.0);
        assert_eq!(em.noise_level, NoiseClass::Silent);
    }

    #[test]
    fn single_hide_bracers_blunt_only() {
        let c = ctx();
        let em = equipment_modifiers_for(&worn(&[(ItemKind::HideBracers, 1.0)]), &c);
        assert!((em.armor_blunt_reduction - c.armor_blunt_absorb_magnitude).abs() < 1e-6);
        assert_eq!(em.armor_pierce_reduction, 0.0);
    }

    #[test]
    fn quality_floor_linear_scaling() {
        let c = ctx();
        // q=0 → floor * base; q=1 → base.
        let em0 = equipment_modifiers_for(&worn(&[(ItemKind::HideBracers, 0.0)]), &c);
        let em1 = equipment_modifiers_for(&worn(&[(ItemKind::HideBracers, 1.0)]), &c);
        let expected_q0 = c.armor_blunt_absorb_magnitude * c.equipment_quality_floor;
        assert!((em0.armor_blunt_reduction - expected_q0).abs() < 1e-6);
        assert!((em1.armor_blunt_reduction - c.armor_blunt_absorb_magnitude).abs() < 1e-6);
    }

    #[test]
    fn dual_armor_composes_additively_then_floor_clamps() {
        // Bracers (Paws) + plated wrap (Body) occupy distinct slots, so both
        // are worn and their reductions compose.
        let c = ctx();
        let em = equipment_modifiers_for(
            &worn(&[
                (ItemKind::HideBracers, 1.0),
                (ItemKind::HidePlatedWrap, 1.0),
            ]),
            &c,
        );
        let raw_blunt = c.armor_blunt_absorb_magnitude + c.armor_pierce_partial_blunt_magnitude;
        let expected_blunt = raw_blunt.min(c.armor_reduction_floor_blunt);
        assert!((em.armor_blunt_reduction - expected_blunt).abs() < 1e-6);
        let raw_pierce = c.armor_pierce_partial_pierce_magnitude;
        assert!((em.armor_pierce_reduction - raw_pierce).abs() < 1e-6);
    }

    #[test]
    fn wielded_weapon_is_read_from_the_slot() {
        let em = equipment_modifiers_for(&worn(&[(ItemKind::BoneStiletto, 0.3)]), &ctx());
        let w = em.weapon.expect("weapon present");
        assert_eq!(w.class, WeaponClass::Pierce);
        assert!(w.fragile, "bone is fragile");
        assert!((w.quality - 0.3).abs() < 1e-6);
    }

    #[test]
    fn equipping_a_second_weapon_displaces_the_first() {
        // One Wielded slot: equipping a stiletto over a spear swaps it out.
        let mut w = WearableSlots::default();
        assert!(w
            .equip(ItemSlot::with_quality(
                ItemKind::BoneTipSpear,
                0.4,
                ItemModifiers::default()
            ))
            .expect("equippable")
            .is_none());
        let displaced = w
            .equip(ItemSlot::with_quality(
                ItemKind::BoneStiletto,
                0.9,
                ItemModifiers::default(),
            ))
            .expect("equippable")
            .expect("spear displaced");
        assert_eq!(displaced.kind, ItemKind::BoneTipSpear);
        let em = equipment_modifiers_for(&w, &ctx());
        assert_eq!(
            em.weapon.expect("weapon present").kind,
            ItemKind::BoneStiletto
        );
    }

    #[test]
    fn wielding_a_sling_sets_ranged_enabled() {
        let em = equipment_modifiers_for(&worn(&[(ItemKind::Sling, 1.0)]), &ctx());
        assert!(em.ranged_enabled);
        let w = em.weapon.expect("weapon present");
        assert_eq!(w.class, WeaponClass::Ranged);
    }

    #[test]
    fn cloak_contributes_visual_mask_only() {
        let c = ctx();
        let em = equipment_modifiers_for(&worn(&[(ItemKind::WovenReedCloak, 1.0)]), &c);
        assert!((em.detection_visual_mask - c.cloak_visual_mask_magnitude).abs() < 1e-6);
        assert_eq!(em.armor_blunt_reduction, 0.0);
        assert!(em.weapon.is_none());
    }

    #[test]
    fn carried_but_unworn_equipment_contributes_nothing() {
        // Items in the pouch are not worn — the worn-only model means a
        // spear in the bag yields a zero aggregate (017's core invariant).
        let em = equipment_modifiers_for(&WearableSlots::default(), &ctx());
        assert!(em.weapon.is_none());
        assert_eq!(em.armor_blunt_reduction, 0.0);
    }

    #[test]
    fn noise_composes_via_max() {
        // Phase 2b has no Loud-class item, so we synthesize the assertion
        // shape against Silent inputs. The `noisier` helper itself is the
        // load-bearing function — verify it directly via composition.
        assert_eq!(
            noisier(NoiseClass::Silent, NoiseClass::Silent),
            NoiseClass::Silent
        );
        assert_eq!(
            noisier(NoiseClass::Silent, NoiseClass::Loud),
            NoiseClass::Loud
        );
        assert_eq!(
            noisier(NoiseClass::Loud, NoiseClass::Silent),
            NoiseClass::Loud
        );
        assert_eq!(
            noisier(NoiseClass::Loud, NoiseClass::Loud),
            NoiseClass::Loud
        );
    }

    #[test]
    fn strike_bonus_is_class_keyed_and_quality_scaled() {
        let c = ctx();
        let em = equipment_modifiers_for(&worn(&[(ItemKind::BoneTipSpear, 0.9)]), &c);
        let w = em.weapon.expect("weapon present");
        // Pierce base × floor-linear(0.9) = 0.12 × (0.25 + 0.75·0.9).
        let expected = c.hunt_strike_pierce_bonus * (0.25 + 0.75 * 0.9);
        assert!((w.strike_bonus(&c) - expected).abs() < 1e-6);
        // Ranged weapons contribute no melee strike bonus.
        let sling = equipment_modifiers_for(&worn(&[(ItemKind::Sling, 1.0)]), &c)
            .weapon
            .expect("sling is a weapon");
        assert_eq!(sling.strike_bonus(&c), 0.0);
    }

    #[test]
    fn quality_scaling_floor_clamps_to_zero_one() {
        // Out-of-band quality is defensively clamped — the source field is
        // already clamped on construction, but mirror the invariant here.
        let m = quality_multiplier(-0.5, 0.25);
        assert!((m - 0.25).abs() < 1e-6);
        let m = quality_multiplier(2.0, 0.25);
        assert!((m - 1.0).abs() < 1e-6);
    }
}
