//! Equipment property substrate (ticket 369 — 016 Phase 2b).
//!
//! Material-property classification read by hunt-strike, combat,
//! ranged-attack, movement-detection, and noise resolvers. Per
//! `docs/systems/crafting.md` §Design constraints: the resolver
//! reads the carried-item identity to decide outcome shape; no
//! numeric modifier fields live on the item itself.
//!
//! Five classifiers (`equip_material`, `weapon_class`, `armor_class`,
//! `noise_class`, `durability_tier`) extend [`ItemKind`] via
//! exhaustive `match` — adding a new variant to `ItemKind` is a
//! compile error until each method is updated, per the
//! compile-time-contracts pillar in CLAUDE.md. The cost is verbose
//! match arms; the payoff is "a new weapon variant cannot be
//! silently invisible to hunt-strike."
//!
//! Pre-017 limitation: any cat carrying an equipment item reads as
//! "wearing" / "wielding" it for resolver purposes. Slot-aware
//! semantics (held vs equipped vs stowed) lands with the
//! slot-inventory follow-on; until then, `Inventory.has(kind)` is
//! the sole gate.

use crate::components::items::ItemKind;
use crate::components::magic::ItemSlot;
use bevy_ecs::prelude::Component;
use std::collections::BTreeMap;

/// Material class for crafted equipment items. Names the substance,
/// which downstream resolvers consult to choose pierce / slash /
/// blunt / silent / fragile behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EquipMaterial {
    /// Bone tip / shaft / blade — pierce-capable, light, fragile
    /// (snaps on lateral load via the hunt-strike snap branch).
    Bone,
    /// Knapped flint — slash-capable, silent; chips rather than
    /// snaps, so it degrades on long timescales rather than via a
    /// single-strike failure mode.
    Flint,
    /// Tanned hide / pelt — quiet, absorbs blunt impact, no pierce
    /// resistance, no snap risk.
    CuredHide,
    /// Woven fiber (reed, sinew, plant fiber) — light, silent, no
    /// armor value; the substrate for stealth garments and ranged
    /// kit (sling cradle, cloak weave).
    Fiber,
}

/// Strike-shape class for weapon items. Read by `resolve_engage_prey`
/// and the take-damage resolver to choose damage curve + reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WeaponClass {
    /// Single-point penetrating strike. Extended reach, fragile.
    Pierce,
    /// Cutting edge across a wound line. Better against bulky prey
    /// than pierce; chips rather than snaps.
    Slash,
    /// Crushing impact. Good against bulky prey; brittle against
    /// hard targets (teeth chip).
    Blunt,
    /// Projectile launcher. Engagement at range; ammunition is
    /// ambient fieldstone (no consumed-ammo entity in Phase 2b).
    Ranged,
}

/// Armor class for wearable defensive equipment. Composes additively
/// in `damage_to_body_part` — a cat may wear multiple armor items
/// and stack their reductions, bounded by the resolver's per-strike
/// floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArmorClass {
    /// Reduces incoming blunt damage by a constant fraction. No
    /// effect on pierce strikes.
    BluntAbsorb,
    /// Reduces both blunt fully and pierce partially. The heavier
    /// armor variant.
    PiercePartial,
}

/// Acoustic profile while carried/worn. Reserved for the metal items
/// in Phase 2c (370) that will read as `Loud` and trigger prey
/// detection penalties; all Phase 2b kit items are `Silent` because
/// the crafting.md table calls them out as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NoiseClass {
    /// No detection penalty from carriage.
    Silent,
    /// Metal-on-metal or stone-on-metal contact triggers a
    /// detection bonus for nearby prey / hostile wildlife.
    Loud,
}

/// Wear-and-tear tier. `Fragile` items roll a snap on failed
/// hunt-strikes; `Durable` items resist that snap entirely;
/// `Standard` sits in the middle (degrades over many strikes, no
/// per-strike snap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DurabilityTier {
    /// Bone weapons — single-strike snap risk on miss.
    Fragile,
    /// Flint / sling / cloak — chips over time, no snap.
    Standard,
    /// Cured-hide armor — resists snap entirely; degrades slowly.
    Durable,
}

/// Anatomical equip slot for a worn item (ticket 017). The OSRS-style
/// "worn" half of the slot-inventory model: each slot holds at most one
/// item ([`WearableSlots`]), and `equipment_modifiers_for` reads only
/// these slots. Slots map onto the [`crate::components::body_zones::BodyPart`]
/// anatomy (ticket 095) — `Wielded`/`Paws` ride the paws, `Cape`/`Body` the
/// flanks. `Collar`/`Ear`/`Tail` are reserved for the Phase 3 adornment
/// producer (370); declared now so the enum is stable, populated then.
///
/// `Ord` is derived and load-bearing: [`WearableSlots`] iterates worn items
/// in `EquipSlot` order, and that order feeds tie-breaking in downstream
/// modifier aggregation — keep variants in a deterministic declaration order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum EquipSlot {
    /// Held weapon — spear, stiletto, club, blade, sling.
    Wielded,
    /// Paw-worn light armor — hide bracers.
    Paws,
    /// Torso armor — hide-plated wrap.
    Body,
    /// Cloak / drape over the flanks — woven reed cloak.
    Cape,
    /// Reserved for Phase 3 adornment (370): woven collar, charm, pendant.
    Collar,
    /// Reserved for Phase 3 adornment (370): notched ear tag.
    Ear,
    /// Reserved for Phase 3 adornment (370): tail ribbon, mentorship tag.
    Tail,
}

/// A cat's worn gear — the OSRS-style equipped half of the slot-inventory
/// model (ticket 017). At most one item per [`EquipSlot`]. Crafting a
/// wearable auto-equips it here (see `craft_at_workshop` /
/// `craft_at_tanning_frame`); deliberate don/doff/swap is ticket 334's
/// `WearItem` resolver. `equipment_modifiers_for` reads only these slots —
/// an item sitting in the pouch is carried, not worn, and contributes
/// nothing to combat/hunt/stealth modifiers.
#[derive(Component, Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WearableSlots {
    /// Deterministic-iteration map (BTreeMap by `EquipSlot` order).
    worn: BTreeMap<EquipSlot, ItemSlot>,
}

impl WearableSlots {
    /// Equip `item` into the slot its kind maps to. Returns the previously
    /// worn item (if the slot was occupied) so the caller can route it back
    /// to the pouch, or `None` if the slot was empty. Returns `Err(item)` if
    /// the item's kind isn't equippable (no [`EquipSlot`]) — caller keeps it
    /// in the pouch.
    pub fn equip(&mut self, item: ItemSlot) -> Result<Option<ItemSlot>, ItemSlot> {
        match item.kind.equip_slot() {
            Some(slot) => Ok(self.worn.insert(slot, item)),
            None => Err(item),
        }
    }

    /// The item worn in `slot`, if any.
    pub fn get(&self, slot: EquipSlot) -> Option<&ItemSlot> {
        self.worn.get(&slot)
    }

    /// Remove and return the item worn in `slot`, if any (334's doff path).
    pub fn take(&mut self, slot: EquipSlot) -> Option<ItemSlot> {
        self.worn.remove(&slot)
    }

    /// Iterate worn items in deterministic `EquipSlot` order.
    pub fn worn_iter(&self) -> impl Iterator<Item = &ItemSlot> {
        self.worn.values()
    }

    /// Whether any item is worn at all.
    pub fn is_empty(&self) -> bool {
        self.worn.is_empty()
    }
}

impl ItemKind {
    /// The anatomical [`EquipSlot`] this item is worn in, or `None` for
    /// non-wearables (consumables, materials, curios, build items).
    ///
    /// **Exhaustive match** — adding a new `ItemKind` is a compile error
    /// until its slot (or `None`) is declared, mirroring `equip_material`
    /// and the compile-time-contracts pillar in CLAUDE.md. A future
    /// adornment (370) declares its `Collar`/`Ear`/`Tail` slot here.
    pub fn equip_slot(self) -> Option<EquipSlot> {
        match self {
            // Wielded weapons.
            Self::BoneTipSpear
            | Self::BoneStiletto
            | Self::ToothNotchedClub
            | Self::FlintBlade
            | Self::Sling => Some(EquipSlot::Wielded),
            // Paw-worn light armor.
            Self::HideBracers => Some(EquipSlot::Paws),
            // Torso armor.
            Self::HidePlatedWrap => Some(EquipSlot::Body),
            // Cloak.
            Self::WovenReedCloak => Some(EquipSlot::Cape),

            // Non-wearable items.
            Self::RawMouse
            | Self::RawRat
            | Self::RawRabbit
            | Self::RawFish
            | Self::RawBird
            | Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid
            | Self::ShinyPebble
            | Self::GlassShard
            | Self::ColorfulShell
            | Self::ShadowBone
            | Self::Barrel
            | Self::Crate
            | Self::Shelf
            | Self::Wood
            | Self::Stone
            | Self::RemedyHealingPoultice
            | Self::RemedyEnergyTonic
            | Self::RemedyMoodTonic
            | Self::RawOrgan
            | Self::DriedFish
            | Self::SmokedMeat
            | Self::PreservedOrgan
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            | Self::Twig
            | Self::Bristle
            | Self::Fiber
            | Self::Flower
            | Self::PolishedStone
            | Self::GroomingBrush
            | Self::PlayBundle
            | Self::CourtshipGift => None,
        }
    }

    /// Equipment material class. `Some(_)` for Phase 2b warrior's-kit
    /// items; `None` for everything else (raw food, herbs,
    /// curiosities, build materials, remedies, behavioral tools).
    ///
    /// **Exhaustive match** — adding a new `ItemKind` variant is a
    /// compile error until classified. Per CLAUDE.md
    /// "Prefer compile-time contracts to runtime checks" — a future
    /// metal weapon ticket (370) must explicitly declare its
    /// material here rather than silently default to `None`.
    pub fn equip_material(self) -> Option<EquipMaterial> {
        match self {
            Self::BoneTipSpear | Self::BoneStiletto | Self::ToothNotchedClub => {
                Some(EquipMaterial::Bone)
            }
            Self::FlintBlade => Some(EquipMaterial::Flint),
            Self::HideBracers | Self::HidePlatedWrap => Some(EquipMaterial::CuredHide),
            Self::Sling | Self::WovenReedCloak => Some(EquipMaterial::Fiber),

            // Non-equipment items — no material classification.
            Self::RawMouse
            | Self::RawRat
            | Self::RawRabbit
            | Self::RawFish
            | Self::RawBird
            | Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid
            | Self::ShinyPebble
            | Self::GlassShard
            | Self::ColorfulShell
            | Self::ShadowBone
            | Self::Barrel
            | Self::Crate
            | Self::Shelf
            | Self::Wood
            | Self::Stone
            | Self::RemedyHealingPoultice
            | Self::RemedyEnergyTonic
            | Self::RemedyMoodTonic
            | Self::RawOrgan
            | Self::DriedFish
            | Self::SmokedMeat
            | Self::PreservedOrgan
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            | Self::Twig
            | Self::Bristle
            | Self::Fiber
            | Self::Flower
            | Self::PolishedStone
            | Self::GroomingBrush
            | Self::PlayBundle
            | Self::CourtshipGift => None,
        }
    }

    /// Strike-shape class — `Some(_)` for weapon items, `None`
    /// otherwise. Hunt-strike / take-damage resolvers consult this
    /// when the carrier swings or is struck.
    pub fn weapon_class(self) -> Option<WeaponClass> {
        match self {
            Self::BoneTipSpear | Self::BoneStiletto => Some(WeaponClass::Pierce),
            Self::FlintBlade => Some(WeaponClass::Slash),
            Self::ToothNotchedClub => Some(WeaponClass::Blunt),
            Self::Sling => Some(WeaponClass::Ranged),

            // Non-weapon items (including the hide armors + the
            // reed cloak — those compose via `armor_class` and the
            // detection / noise resolvers, not the strike resolver).
            Self::HideBracers
            | Self::HidePlatedWrap
            | Self::WovenReedCloak
            | Self::RawMouse
            | Self::RawRat
            | Self::RawRabbit
            | Self::RawFish
            | Self::RawBird
            | Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid
            | Self::ShinyPebble
            | Self::GlassShard
            | Self::ColorfulShell
            | Self::ShadowBone
            | Self::Barrel
            | Self::Crate
            | Self::Shelf
            | Self::Wood
            | Self::Stone
            | Self::RemedyHealingPoultice
            | Self::RemedyEnergyTonic
            | Self::RemedyMoodTonic
            | Self::RawOrgan
            | Self::DriedFish
            | Self::SmokedMeat
            | Self::PreservedOrgan
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            | Self::Twig
            | Self::Bristle
            | Self::Fiber
            | Self::Flower
            | Self::PolishedStone
            | Self::GroomingBrush
            | Self::PlayBundle
            | Self::CourtshipGift => None,
        }
    }

    /// Armor class — `Some(_)` for wearable defensive equipment,
    /// `None` for non-armor items. Composes additively in
    /// `damage_to_body_part`.
    pub fn armor_class(self) -> Option<ArmorClass> {
        match self {
            Self::HideBracers => Some(ArmorClass::BluntAbsorb),
            Self::HidePlatedWrap => Some(ArmorClass::PiercePartial),

            // Non-armor items (including the weapons + the reed
            // cloak — cloak's effect is movement-detection / noise,
            // not damage reduction).
            Self::BoneTipSpear
            | Self::BoneStiletto
            | Self::FlintBlade
            | Self::Sling
            | Self::WovenReedCloak
            | Self::ToothNotchedClub
            | Self::RawMouse
            | Self::RawRat
            | Self::RawRabbit
            | Self::RawFish
            | Self::RawBird
            | Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid
            | Self::ShinyPebble
            | Self::GlassShard
            | Self::ColorfulShell
            | Self::ShadowBone
            | Self::Barrel
            | Self::Crate
            | Self::Shelf
            | Self::Wood
            | Self::Stone
            | Self::RemedyHealingPoultice
            | Self::RemedyEnergyTonic
            | Self::RemedyMoodTonic
            | Self::RawOrgan
            | Self::DriedFish
            | Self::SmokedMeat
            | Self::PreservedOrgan
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            | Self::Twig
            | Self::Bristle
            | Self::Fiber
            | Self::Flower
            | Self::PolishedStone
            | Self::GroomingBrush
            | Self::PlayBundle
            | Self::CourtshipGift => None,
        }
    }

    /// Acoustic profile when carried. All Phase 2b kit items + every
    /// existing item are `Silent` because there are no metal items
    /// yet. Phase 2c (370) flips metal-bearing variants to `Loud`;
    /// the detection / noise resolvers consult this method to apply
    /// the detection-bonus penalty.
    pub fn noise_class(self) -> NoiseClass {
        match self {
            // All current ItemKind variants are silent. The exhaustive
            // match means adding a Loud-class item in 370 is a compile
            // error until classified — the read site (movement /
            // detection resolver) already exists in 369, so 370 just
            // ticks a variant from Silent → Loud.
            Self::BoneTipSpear
            | Self::BoneStiletto
            | Self::FlintBlade
            | Self::HideBracers
            | Self::HidePlatedWrap
            | Self::Sling
            | Self::WovenReedCloak
            | Self::ToothNotchedClub
            | Self::RawMouse
            | Self::RawRat
            | Self::RawRabbit
            | Self::RawFish
            | Self::RawBird
            | Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid
            | Self::ShinyPebble
            | Self::GlassShard
            | Self::ColorfulShell
            | Self::ShadowBone
            | Self::Barrel
            | Self::Crate
            | Self::Shelf
            | Self::Wood
            | Self::Stone
            | Self::RemedyHealingPoultice
            | Self::RemedyEnergyTonic
            | Self::RemedyMoodTonic
            | Self::RawOrgan
            | Self::DriedFish
            | Self::SmokedMeat
            | Self::PreservedOrgan
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            | Self::Twig
            | Self::Bristle
            | Self::Fiber
            | Self::Flower
            | Self::PolishedStone
            | Self::GroomingBrush
            | Self::PlayBundle
            | Self::CourtshipGift => NoiseClass::Silent,
        }
    }

    /// Wear tier — drives the snap-on-failed-strike branch for
    /// `Fragile` items. `Standard` is the implicit-default for most
    /// items; only the Bone weapons declare `Fragile` and only the
    /// hide armors declare `Durable`.
    pub fn durability_tier(self) -> DurabilityTier {
        match self {
            Self::BoneTipSpear | Self::BoneStiletto | Self::ToothNotchedClub => {
                DurabilityTier::Fragile
            }
            Self::HideBracers | Self::HidePlatedWrap => DurabilityTier::Durable,

            // Standard tier — chips or wears over time, no per-strike snap.
            Self::FlintBlade
            | Self::Sling
            | Self::WovenReedCloak
            | Self::RawMouse
            | Self::RawRat
            | Self::RawRabbit
            | Self::RawFish
            | Self::RawBird
            | Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid
            | Self::ShinyPebble
            | Self::GlassShard
            | Self::ColorfulShell
            | Self::ShadowBone
            | Self::Barrel
            | Self::Crate
            | Self::Shelf
            | Self::Wood
            | Self::Stone
            | Self::RemedyHealingPoultice
            | Self::RemedyEnergyTonic
            | Self::RemedyMoodTonic
            | Self::RawOrgan
            | Self::DriedFish
            | Self::SmokedMeat
            | Self::PreservedOrgan
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            | Self::Twig
            | Self::Bristle
            | Self::Fiber
            | Self::Flower
            | Self::PolishedStone
            | Self::GroomingBrush
            | Self::PlayBundle
            | Self::CourtshipGift => DurabilityTier::Standard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::ItemModifiers;

    #[test]
    fn equip_slot_maps_every_kit_item() {
        assert_eq!(
            ItemKind::BoneTipSpear.equip_slot(),
            Some(EquipSlot::Wielded)
        );
        assert_eq!(
            ItemKind::BoneStiletto.equip_slot(),
            Some(EquipSlot::Wielded)
        );
        assert_eq!(
            ItemKind::ToothNotchedClub.equip_slot(),
            Some(EquipSlot::Wielded)
        );
        assert_eq!(ItemKind::FlintBlade.equip_slot(), Some(EquipSlot::Wielded));
        assert_eq!(ItemKind::Sling.equip_slot(), Some(EquipSlot::Wielded));
        assert_eq!(ItemKind::HideBracers.equip_slot(), Some(EquipSlot::Paws));
        assert_eq!(ItemKind::HidePlatedWrap.equip_slot(), Some(EquipSlot::Body));
        assert_eq!(ItemKind::WovenReedCloak.equip_slot(), Some(EquipSlot::Cape));
    }

    #[test]
    fn equip_slot_is_none_for_non_wearables() {
        assert_eq!(ItemKind::RawFish.equip_slot(), None);
        assert_eq!(ItemKind::HerbHealingMoss.equip_slot(), None);
        assert_eq!(ItemKind::Wood.equip_slot(), None);
        assert_eq!(ItemKind::GroomingBrush.equip_slot(), None);
    }

    #[test]
    fn equip_into_empty_slot_returns_no_displacement() {
        let mut w = WearableSlots::default();
        let prior = w
            .equip(ItemSlot::new(
                ItemKind::WovenReedCloak,
                ItemModifiers::default(),
            ))
            .expect("cloak is equippable");
        assert!(prior.is_none());
        assert_eq!(
            w.get(EquipSlot::Cape).map(|s| s.kind),
            Some(ItemKind::WovenReedCloak)
        );
    }

    #[test]
    fn equip_same_slot_displaces_and_returns_prior() {
        let mut w = WearableSlots::default();
        w.equip(ItemSlot::new(
            ItemKind::BoneTipSpear,
            ItemModifiers::default(),
        ))
        .unwrap();
        let displaced = w
            .equip(ItemSlot::new(
                ItemKind::FlintBlade,
                ItemModifiers::default(),
            ))
            .unwrap()
            .expect("spear displaced from the wielded slot");
        assert_eq!(displaced.kind, ItemKind::BoneTipSpear);
        assert_eq!(
            w.get(EquipSlot::Wielded).map(|s| s.kind),
            Some(ItemKind::FlintBlade)
        );
    }

    #[test]
    fn equip_rejects_non_wearable() {
        let mut w = WearableSlots::default();
        let err = w.equip(ItemSlot::new(ItemKind::RawFish, ItemModifiers::default()));
        assert!(err.is_err(), "raw fish is not equippable");
        assert!(w.is_empty());
    }

    #[test]
    fn worn_iter_is_deterministic_by_slot_order() {
        let mut w = WearableSlots::default();
        // Insert out of slot order; iteration must follow EquipSlot order.
        w.equip(ItemSlot::new(
            ItemKind::WovenReedCloak,
            ItemModifiers::default(),
        ))
        .unwrap();
        w.equip(ItemSlot::new(
            ItemKind::BoneTipSpear,
            ItemModifiers::default(),
        ))
        .unwrap();
        w.equip(ItemSlot::new(
            ItemKind::HideBracers,
            ItemModifiers::default(),
        ))
        .unwrap();
        let kinds: Vec<_> = w.worn_iter().map(|s| s.kind).collect();
        // Wielded < Paws < Cape per declaration order.
        assert_eq!(
            kinds,
            vec![
                ItemKind::BoneTipSpear,
                ItemKind::HideBracers,
                ItemKind::WovenReedCloak
            ]
        );
    }

    #[test]
    fn warriors_kit_material_table_matches_crafting_doc() {
        // crafting.md Phase 2b material-property table.
        assert_eq!(
            ItemKind::BoneTipSpear.equip_material(),
            Some(EquipMaterial::Bone)
        );
        assert_eq!(
            ItemKind::BoneStiletto.equip_material(),
            Some(EquipMaterial::Bone)
        );
        assert_eq!(
            ItemKind::ToothNotchedClub.equip_material(),
            Some(EquipMaterial::Bone)
        );
        assert_eq!(
            ItemKind::FlintBlade.equip_material(),
            Some(EquipMaterial::Flint)
        );
        assert_eq!(
            ItemKind::HideBracers.equip_material(),
            Some(EquipMaterial::CuredHide)
        );
        assert_eq!(
            ItemKind::HidePlatedWrap.equip_material(),
            Some(EquipMaterial::CuredHide)
        );
        assert_eq!(ItemKind::Sling.equip_material(), Some(EquipMaterial::Fiber));
        assert_eq!(
            ItemKind::WovenReedCloak.equip_material(),
            Some(EquipMaterial::Fiber)
        );
    }

    #[test]
    fn weapon_classes_match_crafting_doc() {
        assert_eq!(
            ItemKind::BoneTipSpear.weapon_class(),
            Some(WeaponClass::Pierce)
        );
        assert_eq!(
            ItemKind::BoneStiletto.weapon_class(),
            Some(WeaponClass::Pierce)
        );
        assert_eq!(
            ItemKind::FlintBlade.weapon_class(),
            Some(WeaponClass::Slash)
        );
        assert_eq!(
            ItemKind::ToothNotchedClub.weapon_class(),
            Some(WeaponClass::Blunt)
        );
        assert_eq!(ItemKind::Sling.weapon_class(), Some(WeaponClass::Ranged));
        // Cloak + hide armor are not weapons.
        assert_eq!(ItemKind::WovenReedCloak.weapon_class(), None);
        assert_eq!(ItemKind::HideBracers.weapon_class(), None);
        assert_eq!(ItemKind::HidePlatedWrap.weapon_class(), None);
    }

    #[test]
    fn armor_classes_match_crafting_doc() {
        assert_eq!(
            ItemKind::HideBracers.armor_class(),
            Some(ArmorClass::BluntAbsorb)
        );
        assert_eq!(
            ItemKind::HidePlatedWrap.armor_class(),
            Some(ArmorClass::PiercePartial)
        );
        // Cloak is not armor — it composes via detection / noise.
        assert_eq!(ItemKind::WovenReedCloak.armor_class(), None);
        // Weapons are not armor.
        assert_eq!(ItemKind::BoneTipSpear.armor_class(), None);
    }

    #[test]
    fn fragile_tier_covers_bone_weapons_only() {
        for k in [
            ItemKind::BoneTipSpear,
            ItemKind::BoneStiletto,
            ItemKind::ToothNotchedClub,
        ] {
            assert_eq!(k.durability_tier(), DurabilityTier::Fragile);
        }
        // Flint chips rather than snaps — Standard, not Fragile.
        assert_eq!(
            ItemKind::FlintBlade.durability_tier(),
            DurabilityTier::Standard
        );
        // Hide armor is Durable.
        assert_eq!(
            ItemKind::HideBracers.durability_tier(),
            DurabilityTier::Durable
        );
        assert_eq!(
            ItemKind::HidePlatedWrap.durability_tier(),
            DurabilityTier::Durable
        );
        // Sling + cloak are Standard.
        assert_eq!(ItemKind::Sling.durability_tier(), DurabilityTier::Standard);
        assert_eq!(
            ItemKind::WovenReedCloak.durability_tier(),
            DurabilityTier::Standard
        );
    }

    #[test]
    fn all_phase_2b_items_are_silent() {
        // No metal items yet; the entire 2b kit reads as Silent.
        // Phase 2c (370) flips metal variants to Loud — the read
        // sites in detection / noise resolvers are already wired
        // (in 369), so 370 just changes this match arm.
        for k in [
            ItemKind::BoneTipSpear,
            ItemKind::BoneStiletto,
            ItemKind::FlintBlade,
            ItemKind::HideBracers,
            ItemKind::HidePlatedWrap,
            ItemKind::Sling,
            ItemKind::WovenReedCloak,
            ItemKind::ToothNotchedClub,
        ] {
            assert_eq!(k.noise_class(), NoiseClass::Silent);
        }
    }

    #[test]
    fn non_equipment_items_return_none() {
        // Spot-check a few non-equipment items return None for all
        // property classifiers — the exhaustive match guarantees
        // this for every variant, but the assertions document the
        // intent.
        for k in [
            ItemKind::RawMouse,
            ItemKind::Berries,
            ItemKind::HerbCatnip,
            ItemKind::Wood,
            ItemKind::Bone,
            ItemKind::GroomingBrush,
        ] {
            assert_eq!(k.equip_material(), None);
            assert_eq!(k.weapon_class(), None);
            assert_eq!(k.armor_class(), None);
            assert_eq!(k.noise_class(), NoiseClass::Silent);
            assert_eq!(k.durability_tier(), DurabilityTier::Standard);
        }
    }
}
