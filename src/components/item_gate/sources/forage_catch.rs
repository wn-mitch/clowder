//! Forage catch — a foraged plant / root / berry item.
//!
//! Fires once per successful forage at `goap.rs::9964` (canonical) and
//! the legacy duplicate at `disposition.rs::4196`. Like
//! [`super::HuntCatchSource`], the legacy disposition-chain path
//! silently dropped on inventory-full pre-429; the trait's default
//! push-or-overflow body promotes both paths to ground-spawn parity.

use crate::components::item_gate::ItemSource;
use crate::components::items::{ItemKind, ItemModifiers};
use crate::resources::system_activation::Feature;

pub struct ForageCatchSource {
    pub kind: ItemKind,
    pub modifiers: ItemModifiers,
}

impl ItemSource for ForageCatchSource {
    const FEATURE: Feature = Feature::ItemSourcedFromForageCatch;

    fn kind(&self) -> ItemKind {
        self.kind
    }

    fn modifiers(&self) -> ItemModifiers {
        self.modifiers
    }
}
