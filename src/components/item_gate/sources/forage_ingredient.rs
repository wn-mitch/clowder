//! Forage ingredient — herbcraft-ingredient drop (Twig / Fiber / Flower).
//!
//! Fires inside `resolve_forage_item`'s ingredient arm when the
//! forager's tile rolls an ingredient drop. Distinct from the primary
//! forage catch (which uses [`super::ForageCatchSource`]): the cat
//! does **not** pick up the ingredient — it spawns on the forager's
//! tile as an `OnGround` Item for a later herbcrafter to retrieve.
//! Hence `AlwaysGround` placement.

use crate::components::item_gate::{ItemSource, SourcePlacementPolicy};
use crate::components::items::{ItemKind, ItemModifiers};
use crate::resources::system_activation::Feature;

pub struct ForageIngredientSource {
    /// Terrain-derived: Twig for forest, Fiber/Flower for grass.
    pub kind: ItemKind,
    pub modifiers: ItemModifiers,
}

impl ItemSource for ForageIngredientSource {
    const FEATURE: Feature = Feature::ItemSourcedFromForageIngredient;

    fn kind(&self) -> ItemKind {
        self.kind
    }

    fn modifiers(&self) -> ItemModifiers {
        self.modifiers
    }

    fn placement_policy(&self) -> SourcePlacementPolicy {
        SourcePlacementPolicy::AlwaysGround
    }
}
