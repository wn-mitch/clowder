//! Hunt byproduct — a hide / bone / sinew / organ donated by a kill.
//!
//! Fires once per byproduct kind in the engage-prey resolver's
//! byproduct loop at `goap.rs::9476`. Reuses the existing
//! [`Feature::ByproductSpawned`] Positive canary (375) as its gate
//! witness — the producer canary and the items-are-real Source gate are
//! 1:1 by construction (every byproduct push/overflow fires
//! `ByproductSpawned` exactly once), so a second Feature variant would
//! be redundant tracking of the same event.

use crate::components::item_gate::ItemSource;
use crate::components::items::{ItemKind, ItemModifiers};
use crate::resources::system_activation::Feature;

pub struct HuntByproductSource {
    pub kind: ItemKind,
    pub modifiers: ItemModifiers,
}

impl ItemSource for HuntByproductSource {
    const FEATURE: Feature = Feature::ByproductSpawned;

    fn kind(&self) -> ItemKind {
        self.kind
    }

    fn modifiers(&self) -> ItemModifiers {
        self.modifiers
    }
}
