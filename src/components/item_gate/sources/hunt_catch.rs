//! Hunt catch — a single successfully-caught prey carcass.
//!
//! Fires once per catch inside the engage-prey resolver at
//! `goap.rs::9439` (canonical GOAP path) and the duplicate legacy
//! disposition-chain path at `disposition.rs::3757`. Pre-429 the legacy
//! path silently dropped on inventory-full; the trait's default
//! push-or-overflow body promotes both paths to ground-spawn parity.

use crate::components::item_gate::ItemSource;
use crate::components::items::{ItemKind, ItemModifiers};
use crate::resources::system_activation::Feature;

pub struct HuntCatchSource {
    pub kind: ItemKind,
    pub modifiers: ItemModifiers,
}

impl ItemSource for HuntCatchSource {
    const FEATURE: Feature = Feature::ItemSourcedFromHuntCatch;

    fn kind(&self) -> ItemKind {
        self.kind
    }

    fn modifiers(&self) -> ItemModifiers {
        self.modifiers
    }
}
