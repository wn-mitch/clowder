//! Preservation output — drying-rack / smoking-rack completion spawn.
//!
//! Fires once per rack completion in `preservation.rs` when the load
//! finishes its dry/smoke cycle. The output Item (DriedFish /
//! PreservedOrgan) appears on the rack's ground tile and waits for a
//! cat to pick it up — the actor (the loader) never carries the rack's
//! output, so this is an `AlwaysGround` Source.
//!
//! The legacy `Feature::FoodDried` / `Feature::OrganPreserved` emissions
//! stay alongside this as gameplay-event witnesses; the new
//! `ItemSourcedFromPreservation` Feature is the items-are-real gate
//! witness (same separation as `CarcassHarvested` vs
//! `ItemSourcedFromHarvestCarcass`).

use crate::components::item_gate::{ItemSource, SourcePlacementPolicy};
use crate::components::items::{ItemKind, ItemModifiers};
use crate::resources::system_activation::Feature;

pub struct PreservationOutputSource {
    pub kind: ItemKind,
    pub modifiers: ItemModifiers,
    /// Computed from `source_quality` + `crafter_skill` via
    /// `preservation_output_quality(...)`. Rides into the ground spawn
    /// so a future picker reads the actual provenance, not the trait's
    /// default `1.0`.
    pub quality: f32,
}

impl ItemSource for PreservationOutputSource {
    const FEATURE: Feature = Feature::ItemSourcedFromPreservation;

    fn kind(&self) -> ItemKind {
        self.kind
    }

    fn modifiers(&self) -> ItemModifiers {
        self.modifiers
    }

    fn ground_quality(&self) -> f32 {
        self.quality
    }

    fn placement_policy(&self) -> SourcePlacementPolicy {
        SourcePlacementPolicy::AlwaysGround
    }
}
