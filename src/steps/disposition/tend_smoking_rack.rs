//! `TendSmokingRack` — ticket 367, Phase 1b preservation.
//!
//! One tend cycle on a loaded Smoking Rack. Advances
//! `SmokingRackState.progress` by `1.0 / tends_needed`, stamps
//! `last_tended_at_tick`, and (if progress reaches 1.0) spawns the
//! output `SmokedMeat` `Item` entity on the ground at the rack's
//! position and resets the rack to idle.
//!
//! The per-rack cooldown discipline (the
//! `HasLoadedSmokingRackOffCooldown` marker won't fire again on the
//! same rack for ~2 sim-hours after a tend) is enforced at the
//! marker layer in `update_colony_building_markers`. The resolver
//! itself just re-checks that the rack is loaded and off-cooldown
//! defensively.

use bevy::prelude::*;

use crate::components::building::{SmokingRackState, Structure};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::physical::Position;
use crate::resources::sim_constants::CraftingConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `TendSmokingRack`
///
/// **Real-world effect** — increments
/// `SmokingRackState.tends_completed`, advances `progress` by
/// `1.0 / tends_needed`, and stamps `last_tended_at_tick`. When
/// progress reaches 1.0, spawns a `SmokedMeat` `Item` entity at the
/// rack's position (carrying the source meat's quality + modifiers
/// onto the output, items-are-real precedent) and resets the rack
/// to idle.
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::ZoneIs(PlannerZone::SmokingRack)` by
/// `src/ai/planner/actions.rs::tend_smoking_rack_actions`. DSE-side
/// gates: `CanSmoke` + `HasLoadedSmokingRackOffCooldown` + forbid
/// `Incapacitated`.
///
/// **Runtime preconditions** — re-checks that a loaded smoking rack
/// exists in range AND that the per-rack tend cooldown has elapsed.
/// Drift between marker authoring (last tick) and execution (this
/// tick) is possible if multiple cats race to tend the same rack —
/// the loser sees the rack with `last_tended_at_tick == now` and
/// returns `unwitnessed(Fail)`.
///
/// **Witness** — `StepOutcome<Option<bool>>`. Witness is `None`
/// when no tend happened (no eligible rack in range — `Fail`).
/// `Some(false)` when an intermediate tend advanced progress.
/// `Some(true)` when this tend closed out the craft and spawned the
/// output entity. `is_witnessed()` is true for both `Some(_)`
/// variants, so `record_if_witnessed` fires `SmokingRackTended` for
/// every successful tend; the dispatch arm separately routes
/// `Feature::MeatSmoked` on completion via the explicit
/// `Some(true)` check.
///
/// **Feature emission** — caller passes
/// `Feature::SmokingRackTended` (Positive) on intermediate tends
/// and `Feature::MeatSmoked` (Positive) on completion.
pub fn resolve_tend_smoking_rack(
    cat_pos: Position,
    current_tick: u64,
    racks: &mut Query<(Entity, &Position, &Structure, &mut SmokingRackState)>,
    proximity: i32,
    crafting: &CraftingConstants,
    commands: &mut Commands,
) -> StepOutcome<Option<bool>> {
    // Find the nearest loaded smoking rack that's off-cooldown.
    let cooldown = crafting.smoking_tend_cooldown_ticks;
    let mut best: Option<(Entity, i32)> = None;
    for (entity, pos, structure, state) in racks.iter() {
        if structure.kind != crate::components::building::StructureType::SmokingRack {
            continue;
        }
        if structure.effectiveness() == 0.0 || state.loaded.is_none() || state.progress >= 1.0 {
            continue;
        }
        let off_cooldown = state.last_tended_at_tick == 0
            || current_tick.saturating_sub(state.last_tended_at_tick) >= cooldown;
        if !off_cooldown {
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
        return StepOutcome::unwitnessed(StepResult::Fail(
            "no tendable smoking rack in range".into(),
        ));
    };

    // Apply the tend.
    let mut completed = false;
    let mut output_spawn: Option<(Position, f32, crate::components::items::ItemModifiers)> = None;
    for (entity, rack_pos, _, mut state) in racks.iter_mut() {
        if entity != rack_entity {
            continue;
        }
        let Some(load) = state.loaded.clone() else {
            return StepOutcome::unwitnessed(StepResult::Fail(
                "tend target lost its load mid-tick".into(),
            ));
        };
        state.tends_completed = state.tends_completed.saturating_add(1);
        state.last_tended_at_tick = current_tick;
        let needed = state.tends_needed.max(1) as f32;
        state.progress = (state.progress + 1.0 / needed).min(1.0);
        if state.progress >= 1.0 {
            output_spawn = Some((*rack_pos, load.source_quality, load.source_modifiers));
            // Reset the rack to idle on completion.
            state.loaded = None;
            state.fuel_loaded = false;
            state.progress = 0.0;
            state.tends_completed = 0;
            state.last_tended_at_tick = 0;
            completed = true;
        }
        break;
    }

    if let Some((spawn_pos, quality, modifiers)) = output_spawn {
        commands.spawn((
            Item::with_modifiers(ItemKind::SmokedMeat, quality, ItemLocation::OnGround, modifiers),
            spawn_pos,
        ));
    }

    StepOutcome::witnessed_with(StepResult::Advance, completed)
}
