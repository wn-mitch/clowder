//! `LoadSmokingRack` — ticket 367, Phase 1b preservation.
//!
//! Single-tick action. Drains one raw meat slot + one fuel slot
//! (Wood) from the cat's inventory and stamps the load onto the
//! nearest idle Smoking Rack. After loading, smoking progress
//! advances only on subsequent tend cycles
//! (`resolve_tend_smoking_rack`); the rack does not produce
//! `SmokedMeat` autonomously.

use bevy::prelude::*;

use crate::components::building::{SmokingLoad, SmokingRackState, Structure};
use crate::components::items::ItemKind;
use crate::components::magic::Inventory;
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::sim_constants::CraftingConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `LoadSmokingRack`
///
/// **Real-world effect** — drains one `is_raw_meat()` slot and one
/// `Wood` slot from the cat's inventory, stamping the load onto the
/// nearest idle Smoking Rack's `SmokingRackState`. The rack moves
/// into "loaded but un-tended" state (`progress = 0.0`,
/// `tends_completed = 0`, `last_tended_at_tick = 0` sentinel — the
/// first tend can fire as soon as a cat reaches the rack).
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::ZoneIs(PlannerZone::SmokingRack)` by
/// `src/ai/planner/actions.rs::smoking_meat_actions`. DSE-side
/// gates: `CanSmoke` + `HasFunctionalSmokingRack` +
/// `HasSmokeableInInventory` + forbid `Incapacitated`.
///
/// **Runtime preconditions** — re-checks that an idle smoking rack
/// exists in range AND that the cat carries both a meat slot and a
/// fuel slot. Either drift (rack got loaded, cat lost the meat /
/// fuel between planning and execution) returns
/// `unwitnessed(Fail)` so the planner re-picks.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff a real load
/// actually happened (both inputs drained, state flipped). `false`
/// means a runtime check failed — never a silent advance. The
/// witness-bearing shape is required because the caller emits
/// `Feature::MeatLoadedOnSmokingRack` via `record_if_witnessed`,
/// which is only callable on witness-bearing outcomes (see
/// `src/steps/outcome.rs`).
///
/// **Feature emission** — caller passes
/// `Feature::MeatLoadedOnSmokingRack` (Positive) to
/// `record_if_witnessed`. The downstream `Feature::MeatSmoked`
/// fires on the tend cycle that completes the craft.
pub fn resolve_load_smoking_rack(
    cat_pos: Position,
    inventory: &mut Inventory,
    skills: &Skills,
    racks: &mut Query<(Entity, &Position, &Structure, &mut SmokingRackState)>,
    proximity: i32,
    crafting: &CraftingConstants,
) -> StepOutcome<bool> {
    // Find the nearest idle smoking rack within proximity.
    let mut best: Option<(Entity, i32)> = None;
    for (entity, pos, structure, state) in racks.iter() {
        if structure.kind != crate::components::building::StructureType::SmokingRack {
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
        return StepOutcome::unwitnessed(StepResult::Fail("no idle smoking rack in range".into()));
    };

    // Pick a meat kind to consume. The smoking pipeline produces
    // `SmokedMeat` uniformly across mammal / bird sources — see
    // ticket 367's recipe registry, which registers four parallel
    // recipes (smoked.mouse / .rat / .rabbit / .bird) all outputting
    // `ItemKind::SmokedMeat`. We pick the first qualifying slot.
    let meat_idx = inventory.slots.iter().position(|s| s.kind.is_raw_meat());
    let fuel_idx = inventory
        .slots
        .iter()
        .position(|s| s.kind == ItemKind::Wood);
    let (Some(m_idx), Some(_)) = (meat_idx, fuel_idx) else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "cat must carry both raw meat and fuel".into(),
        ));
    };

    // Drain meat first, capturing source identity for the load.
    let meat_slot = inventory.slots.swap_remove(m_idx);
    // Re-locate fuel because the swap_remove may have shifted indices.
    let fuel_idx = inventory
        .slots
        .iter()
        .position(|s| s.kind == ItemKind::Wood);
    if let Some(f_idx) = fuel_idx {
        inventory.slots.swap_remove(f_idx);
    } else {
        // Shouldn't happen — we checked above — but bail rather
        // than leave the rack half-loaded with no fuel.
        return StepOutcome::unwitnessed(StepResult::Fail("fuel vanished mid-load".into()));
    }

    // 367-4b — `ItemSlot.quality` carries the source meat's per-
    // instance quality from pickup. The output entity's final quality
    // is computed at the closing tend cycle by combining this with
    // `crafter_skill` via `CraftingConstants::preservation_quality_*`.
    // The loader-as-crafter convention (rather than last-tender-as-
    // crafter) keeps SmokingRackState's persisted skill scalar — see
    // `SmokingLoad::crafter_skill` doc-comment for the trade-off.
    let crafter_skill =
        (skills.herbcraft * 0.5 + skills.foraging * 0.3 + crafting.preservation_skill_baseline)
            .clamp(0.0, 1.0);
    let load = SmokingLoad {
        source_kind: meat_slot.kind,
        source_quality: meat_slot.quality,
        crafter_skill,
        source_modifiers: meat_slot.modifiers,
    };

    for (entity, _, _, mut state) in racks.iter_mut() {
        if entity == rack_entity {
            state.loaded = Some(load.clone());
            state.fuel_loaded = true;
            state.progress = 0.0;
            state.last_tended_at_tick = 0;
            state.tends_completed = 0;
            state.tends_needed = crafting.smoking_tends_needed.max(1);
            return StepOutcome::witnessed(StepResult::Advance);
        }
    }
    StepOutcome::unwitnessed(StepResult::Fail("rack vanished mid-load".into()))
}
