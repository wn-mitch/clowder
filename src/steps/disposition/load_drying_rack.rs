//! `LoadDryingRack` — ticket 367, Phase 1b preservation.
//!
//! Single-tick action. Picks a recipe based on inventory contents
//! (RawFish → DriedFish; RawOrgan + 1 herb → PreservedOrgan), drains
//! the inputs from inventory, and stamps the chosen load onto the
//! nearest idle Drying Rack's `DryingRackState`. The per-tick
//! preservation system (ticket 367 Commit 5,
//! `systems::preservation`) then advances the rack's `progress`
//! field whenever weather is Clear, spawning the output `Item` when
//! `progress >= 1.0`.

use bevy::prelude::*;

use crate::components::building::{DryingLoad, DryingRackState, DryingRecipe, Structure};
use crate::components::items::{ItemKind, ItemModifiers};
use crate::components::magic::Inventory;
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::sim_constants::CraftingConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `LoadDryingRack`
///
/// **Real-world effect** — drains one drying-eligible input item
/// (and one herb if Preserved Organ is the chosen recipe) from the
/// cat's inventory and loads the nearest idle Drying Rack. After
/// this step, `DryingRackState.loaded = Some(_)` and `progress =
/// 0.0`; the per-tick preservation system advances progress while
/// weather is Clear.
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::ZoneIs(PlannerZone::DryingRack)` by
/// `src/ai/planner/actions.rs::drying_food_actions`. Cat eligibility,
/// station availability, and inventory disjunction are gated upstream
/// at the `DryFoodDse` eligibility filter (`CanDry` plus
/// `HasFunctionalDryingRack` plus `HasDryableInInventory` plus
/// `forbid Incapacitated`).
///
/// **Runtime preconditions** — re-checks that an idle drying rack
/// exists within `proximity` tiles and that the cat actually carries
/// a drying-eligible item. Both checks can drift between planning
/// and execution (another cat may have loaded the same rack; the
/// cat may have dropped or eaten the food en route). On either
/// drift, returns `unwitnessed(Fail)` — the planner re-picks.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff a real load
/// happened (inventory actually drained, rack state actually
/// flipped). `false` means the runtime check failed and no
/// real-world effect occurred — never a silent advance. The
/// witness-bearing shape is required because the caller emits
/// `Feature::FoodLoadedOnDryingRack` via `record_if_witnessed`,
/// which is only callable on witness-bearing outcomes (see
/// `src/steps/outcome.rs`).
///
/// **Feature emission** — caller passes `Feature::FoodLoadedOnDryingRack`
/// (Positive) to `record_if_witnessed`. The downstream
/// `Feature::FoodDried` fires when the per-tick preservation system
/// completes the craft.
pub fn resolve_load_drying_rack(
    cat_pos: Position,
    inventory: &mut Inventory,
    skills: &Skills,
    crafting: &CraftingConstants,
    racks: &mut Query<(Entity, &Position, &Structure, &mut DryingRackState)>,
    proximity: i32,
) -> StepOutcome<bool> {
    // Find the nearest idle drying rack within proximity. Iterating
    // mutably is the standard Bevy pattern; collect a candidate Entity
    // first so the borrow doesn't span the actual load mutation.
    let mut best: Option<(Entity, i32)> = None;
    for (entity, pos, structure, state) in racks.iter() {
        if structure.kind != crate::components::building::StructureType::DryingRack {
            continue;
        }
        if structure.effectiveness() == 0.0 || state.loaded.is_some() {
            continue;
        }
        let d = cat_pos.manhattan_distance(pos);
        if d > proximity {
            continue;
        }
        if best.is_none_or(|(_, cur)| d < cur) {
            best = Some((entity, d));
        }
    }
    let Some((rack_entity, _)) = best else {
        return StepOutcome::unwitnessed(StepResult::Fail("no idle drying rack in range".into()));
    };

    // Pick a recipe. Fish wins over organ on tie because the
    // preservation hypothesis is fish-buffer-heavy (Dried Fish is the
    // 3-day staple); organ recipe needs an extra herb anyway, so we
    // try fish first.
    let (recipe, drain_organ_plus_herb) = if inventory.has_raw_fish() {
        (DryingRecipe::DriedFish, false)
    } else if inventory.has_raw_organ() && inventory.has_any_herb() {
        (DryingRecipe::PreservedOrgan, true)
    } else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "cat carries no dryable food (or organ without a herb)".into(),
        ));
    };

    // Drain inputs and capture the source item's modifiers /
    // quality. Items-are-real precedent: we copy the source's
    // `modifiers` (corruption + from_organ) onto the rack's load so
    // they ride through to the output, matching the inventory-eating
    // pattern (`eat_from_inventory`'s consume-the-slot shape).
    let (source_quality, source_modifiers) = match recipe {
        DryingRecipe::DriedFish => take_kind(inventory, ItemKind::RawFish),
        DryingRecipe::PreservedOrgan => {
            let mods = take_kind(inventory, ItemKind::RawOrgan);
            if drain_organ_plus_herb {
                // Consume one herb (any kind) alongside the organ.
                // The herb is fungible at this layer — the recipe
                // doesn't depend on which herb is used in stage-1.
                let _ = take_any_herb(inventory);
            }
            mods
        }
    };

    // 367-4b — capture the loader's normalised crafter skill. For
    // drying, the loader IS the substrate-correct crafter: sun does
    // the rest of the work without further cat involvement, so there's
    // no per-tend candidate. Composes herbcraft (preservation-adjacent
    // knowledge) and foraging (raw-food handling) under a baseline
    // floor; see `CraftingConstants::preservation_skill_baseline`.
    let crafter_skill = (skills.herbcraft * 0.5
        + skills.foraging * 0.3
        + crafting.preservation_skill_baseline)
        .clamp(0.0, 1.0);

    // Apply the load via a mutable iteration over racks. We re-enter
    // the loop because the previous borrow has dropped.
    for (entity, _, _, mut state) in racks.iter_mut() {
        if entity == rack_entity {
            state.loaded = Some(DryingLoad {
                recipe,
                source_quality,
                crafter_skill,
                source_modifiers,
            });
            state.progress = 0.0;
            return StepOutcome::witnessed(StepResult::Advance);
        }
    }
    // The rack disappeared between scan and write — shouldn't
    // happen within a single tick, but the Fail path is safe.
    StepOutcome::unwitnessed(StepResult::Fail("rack vanished mid-load".into()))
}

/// Drain one instance of `kind` from the cat's inventory; returns
/// the captured quality + modifiers so the rack's load can ride them
/// onto the output entity (367-4b: `ItemSlot` now carries
/// per-instance `quality` propagated from the source `Item.quality`
/// at pickup time).
fn take_kind(inventory: &mut Inventory, kind: ItemKind) -> (f32, ItemModifiers) {
    if let Some(idx) = inventory.slots.iter().position(|s| s.kind == kind) {
        let slot = inventory.slots.swap_remove(idx);
        (slot.quality, slot.modifiers)
    } else {
        (1.0, ItemModifiers::default())
    }
}

/// Drain any one herb from the cat's inventory. Used by the
/// Preserved Organ recipe — the recipe consumes "1 herb" without
/// caring which kind.
fn take_any_herb(inventory: &mut Inventory) -> Option<ItemKind> {
    if let Some(idx) = inventory.slots.iter().position(|s| s.kind.is_herb()) {
        let slot = inventory.slots.swap_remove(idx);
        Some(slot.kind)
    } else {
        None
    }
}
