//! Harvest carcass — the magic-substrate `ShadowBone` yield produced
//! when a cat harvests a corruption-laden carcass.
//!
//! Fires once per harvest inside `goap.rs::GoapActionKind::HarvestCarcass`.
//! Pre-482 the call site used `inventory.add_item_with_modifiers(...)`,
//! whose `false` return on full inventory was silently ignored —
//! ShadowBone vanished when the harvester's pouch was full. The trait's
//! default push-or-overflow body lands the bone on the ground in that
//! corner case, with `OverflowToGround` firing as the canary witness.
//!
//! The existing `Feature::CarcassHarvested` gameplay-event emission
//! stays alongside this — that Feature is the harvest-event witness;
//! `ItemSourcedFromHarvestCarcass` is the items-are-real gate witness
//! (same separation as `FoodDried` vs `ItemSourcedFromPreservation`).

use crate::components::item_gate::ItemSource;
use crate::components::items::{ItemKind, ItemModifiers};
use crate::resources::system_activation::Feature;

pub struct HarvestCarcassSource {
    /// Carries the corruption stamp computed from the harvest tile.
    pub modifiers: ItemModifiers,
}

impl ItemSource for HarvestCarcassSource {
    const FEATURE: Feature = Feature::ItemSourcedFromHarvestCarcass;

    fn kind(&self) -> ItemKind {
        ItemKind::ShadowBone
    }

    fn modifiers(&self) -> ItemModifiers {
        self.modifiers
    }
}
