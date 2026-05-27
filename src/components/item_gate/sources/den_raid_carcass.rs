//! Den-raid carcass — a freshly-killed prey from a successful den raid.
//!
//! Fires once per kill inside the raid loop at `disposition.rs::3234` and
//! the canonical mirror `goap.rs::8837`. Both sites perturb the den
//! center by ±1 tile per kill (so a 3-kill raid spawns three nearby
//! ground carcasses on inventory overflow), which is encoded here via
//! the [`ItemSource::ground_position`] override.

use crate::components::item_gate::ItemSource;
use crate::components::items::{ItemKind, ItemModifiers};
use crate::components::physical::Position;
use crate::resources::system_activation::Feature;

pub struct DenRaidCarcassSource {
    pub kind: ItemKind,
    pub modifiers: ItemModifiers,
    /// `d.den_dropped_item_quality` from `DispositionConstants` — the
    /// quality assigned to ground-overflow carcasses (inventory-push
    /// stays at `1.0`, matching pre-429 behavior).
    pub ground_quality: f32,
    /// Pre-perturbed ground position (caller applies the ±1 RNG
    /// offset around the den center before constructing the Source).
    pub ground_position: Position,
}

impl ItemSource for DenRaidCarcassSource {
    const FEATURE: Feature = Feature::ItemSourcedFromDenRaid;

    fn kind(&self) -> ItemKind {
        self.kind
    }

    fn modifiers(&self) -> ItemModifiers {
        self.modifiers
    }

    fn ground_quality(&self) -> f32 {
        self.ground_quality
    }

    fn ground_position(&self, _default: Position) -> Position {
        self.ground_position
    }
}
