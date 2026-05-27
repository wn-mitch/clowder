//! # `ItemSource` — the items-are-real Source gate (ticket 429)
//!
//! Every item state-transition in Clowder flows through one of three
//! named gates: **Source** (item enters world/Inventory), **Transfer**
//! (item moves without form change), **Sink** (item exits world/Inventory).
//! This module is the Source-side primitive layer; `item_transfer.rs` is
//! the Transfer-side primitive layer; existing function-shape resolvers
//! under `src/steps/disposition/**` carry the Sink contract today.
//!
//! See [`docs/systems/items-are-real.md`](../../../docs/systems/items-are-real.md)
//! for the doctrine and audit table.
//!
//! ## The trait
//!
//! [`ItemSource`] is implemented per origin (DenRaid carcass, hunt catch,
//! hunt byproduct, forage catch — and trader arrival when 381 lands).
//! Each impl declares its identity ([`ItemSource::kind`] /
//! [`ItemSource::modifiers`]) and its narrative anchor ([`ItemSource::FEATURE`]).
//! The default [`ItemSource::source`] body handles **push-or-overflow**:
//! the new item lands in the actor's `Inventory` if there's room, else
//! spawns as a real `Item` entity on the ground at
//! `SourceCtx::default_position` (or a per-Source override via
//! [`ItemSource::ground_position`]).
//!
//! The return type — `StepOutcome<Option<SourcePlacement>>` — composes
//! with `record_if_witnessed` in the standard step-resolver pattern: the
//! caller emits `Self::FEATURE` (always, when the witness is `Some`) and
//! additionally emits [`Feature::OverflowToGround`] when the witness is
//! `Some(SourcePlacement::Ground { .. })`.
//!
//! ## Why a trait
//!
//! Mirrors ticket 438's "prefer compile-time contracts to runtime checks"
//! discipline: registering a Source via the trait makes the
//! kind/modifiers/Feature contract a type-level invariant. Adding a new
//! item-creating event without naming its Feature is a compile error.
//! The CI script `check_item_transfers.sh` is the *backstop* for sites
//! that mutate `inventory.pouch` without going through the trait — see
//! the module doc on `item_transfer.rs` for the parallel three-layer
//! enforcement framing.

use bevy::prelude::*;

use crate::components::items::{Item, ItemKind, ItemLocation, ItemModifiers};
use crate::components::magic::{Inventory, ItemSlot};
use crate::components::physical::Position;
use crate::resources::system_activation::Feature;
use crate::steps::{StepOutcome, StepResult};

pub mod sources;

/// Where the Source placed the new item.
///
/// Returned in the witness so callers can emit secondary Features
/// (e.g., [`Feature::OverflowToGround`] on `Ground`) and downstream
/// systems can read the spawned ground entity.
#[derive(Debug, Clone, Copy)]
pub enum SourcePlacement {
    /// Pushed into the actor's `Inventory.pouch` at the tail slot.
    Inventory { kind: ItemKind },
    /// Spawned as a real `Item` entity at `ItemLocation::OnGround`.
    /// `entity` is the Commands-reserved id (valid post-flush).
    Ground { entity: Entity, kind: ItemKind },
}

/// Context handed to every [`ItemSource`] impl.
///
/// Holds the actor's mutable `Inventory` (for the inventory-push arm),
/// the `Commands` queue (for the ground-overflow spawn arm), and the
/// default spawn position used when the impl doesn't override
/// [`ItemSource::ground_position`].
pub struct SourceCtx<'a, 'w, 's> {
    pub inventory: &'a mut Inventory,
    pub commands: &'a mut Commands<'w, 's>,
    /// Default position for the ground-overflow spawn. Most call sites
    /// pass the actor's current position; den-raid passes the den
    /// center (with the per-iter random perturbation baked into a
    /// per-Source [`ItemSource::ground_position`] override).
    pub default_position: Position,
}

/// Items-are-real Source gate.
///
/// **Real-world effect** — adds one item to the world. The item lands
/// in the actor's `Inventory.pouch` if there's room; otherwise spawns
/// as an `Item` entity on the ground at the default-or-overridden
/// position. Either branch is a witnessed success — silent-drop is not
/// a valid Source outcome (any "the catch was lost" semantics must be
/// expressed as a different gate, e.g. a Sink for crafting waste).
///
/// **Feature emission** — caller passes `Self::FEATURE` to
/// [`StepOutcome::record_if_witnessed`]; additionally records
/// [`Feature::OverflowToGround`] when the witness carries
/// [`SourcePlacement::Ground`]. Both Feature variants are exhaustively
/// classified in `Feature::expected_to_fire_per_soak` so the never-fired
/// canary catches a stuck Source.
pub trait ItemSource {
    /// The Feature variant identifying this Source's origin. Caller
    /// passes this to `record_if_witnessed`.
    const FEATURE: Feature;

    /// What kind of item this Source produces.
    fn kind(&self) -> ItemKind;

    /// Modifiers (corruption, etc.) stamped at creation.
    fn modifiers(&self) -> ItemModifiers;

    /// Quality assigned to the **ground-overflow** spawn. Defaults to
    /// `1.0` (matching pre-429 inline-spawn behavior at most sites).
    /// Den-raid overrides this with `d.den_dropped_item_quality`.
    ///
    /// Inventory-push quality is fixed at `1.0` via `ItemSlot::new`
    /// (the pre-429 behavior — inventory slots have never carried
    /// per-Source quality variance).
    fn ground_quality(&self) -> f32 {
        1.0
    }

    /// Override the ground-overflow spawn position. Default returns
    /// `SourceCtx::default_position` unmodified. Den-raid uses this to
    /// apply the per-iter ±1 random offset around the den center.
    fn ground_position(&self, default: Position) -> Position {
        default
    }

    /// Push the item into Inventory if room, else spawn on the ground.
    /// Default impl handles both arms; override only if the per-Source
    /// semantics genuinely diverge (none today).
    fn source(&self, ctx: &mut SourceCtx<'_, '_, '_>) -> StepOutcome<Option<SourcePlacement>> {
        let kind = self.kind();
        let modifiers = self.modifiers();
        let placement = if !ctx.inventory.is_full() {
            ctx.inventory.pouch.push(ItemSlot::new(kind, modifiers));
            SourcePlacement::Inventory { kind }
        } else {
            let position = self.ground_position(ctx.default_position);
            let entity = ctx
                .commands
                .spawn((
                    Item::with_modifiers(
                        kind,
                        self.ground_quality(),
                        ItemLocation::OnGround,
                        modifiers,
                    ),
                    position,
                ))
                .id();
            SourcePlacement::Ground { entity, kind }
        };
        StepOutcome::witnessed_with(StepResult::Advance, placement)
    }
}
