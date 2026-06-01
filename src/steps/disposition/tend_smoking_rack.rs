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
use crate::components::recipe::{CraftedItem, RecipeId};
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
    proximity: f32,
    crafting: &CraftingConstants,
    commands: &mut Commands,
) -> StepOutcome<Option<bool>> {
    // Find the nearest loaded smoking rack that's off-cooldown.
    let cooldown = crafting.smoking_tend_cooldown_ticks;
    let mut best: Option<(Entity, f32)> = None;
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
        let d = cat_pos.distance_to(pos);
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
    let mut output_spawn: Option<CompletionSpawn> = None;
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
            // 367-4b — compose source quality (the meat's per-instance
            // quality at load time) with the loader's normalised
            // crafter skill via the CraftingConstants formula. See
            // `CraftingConstants::preservation_quality_input_weight`
            // doc-comment for the rationale.
            let output_quality =
                preservation_output_quality(load.source_quality, load.crafter_skill, crafting);
            output_spawn = Some(CompletionSpawn {
                pos: *rack_pos,
                quality: output_quality,
                modifiers: load.source_modifiers,
                recipe_id: recipe_id_for_source(load.source_kind),
            });
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

    if let Some(spawn) = output_spawn {
        commands.spawn((
            Item::with_modifiers(
                ItemKind::SmokedMeat,
                spawn.quality,
                ItemLocation::OnGround,
                spawn.modifiers,
            ),
            spawn.pos,
            CraftedItem {
                recipe: spawn.recipe_id,
                // No per-tender entity carried on the load yet — see
                // `SmokingLoad::crafter_skill` doc-comment for the
                // loader-as-crafter convention. A follow-on can
                // persist the loader's Entity if narrative
                // attribution wants it.
                crafter: None,
                crafted_at_tick: current_tick,
            },
        ));
    }

    StepOutcome::witnessed_with(StepResult::Advance, completed)
}

/// Captured at completion time so the second pass (entity spawn +
/// `CraftedItem` provenance attach) doesn't re-borrow `SmokingRackState`.
struct CompletionSpawn {
    pos: Position,
    quality: f32,
    modifiers: crate::components::items::ItemModifiers,
    recipe_id: RecipeId,
}

/// Map a `SmokingLoad::source_kind` to the matching `preserve.smoked.*`
/// recipe id. Hard-coded mirror of the 4 entries registered in
/// `populate_recipe_registry`; falls back to a debug-shaped string
/// for any defensively-passed non-smokeable kind (shouldn't happen
/// in practice — load resolver gates `is_raw_meat`).
fn recipe_id_for_source(kind: ItemKind) -> RecipeId {
    match kind {
        ItemKind::RawMouse => RecipeId("preserve.smoked.mouse"),
        ItemKind::RawRat => RecipeId("preserve.smoked.rat"),
        ItemKind::RawRabbit => RecipeId("preserve.smoked.rabbit"),
        ItemKind::RawBird => RecipeId("preserve.smoked.bird"),
        // Defensive: load resolver gates `is_raw_meat()`, so any
        // unexpected kind here is a contract violation upstream.
        // Carrying a placeholder rather than panicking keeps the run
        // alive; a future contract assertion can promote this to a
        // hard failure.
        _ => RecipeId("preserve.smoked.unknown"),
    }
}

/// 367-4b — Factorio/RimWorld-style output quality formula.
///
/// Combines the input item's per-instance quality with the loader's
/// normalised crafter skill into a `[0.0, 1.0]` output quality.
/// Used by `resolve_tend_smoking_rack` at completion and by the
/// per-tick `systems::preservation` drying system (Commit 5) at
/// `progress >= 1.0`.
///
/// The formula is intentionally tunable — see
/// `CraftingConstants::preservation_quality_input_weight` /
/// `..._skill_weight` doc-comments for the trade-off space.
///
/// Quality is currently **decorative** (the eat path reads
/// `ItemKind`, not `Item.quality`) — wiring quality into food value
/// is a separate follow-on. The substrate landing here is what makes
/// that follow-on a one-line change at `food_value()` rather than a
/// full pipeline retrofit.
pub fn preservation_output_quality(
    source_quality: f32,
    crafter_skill: f32,
    crafting: &CraftingConstants,
) -> f32 {
    (source_quality * crafting.preservation_quality_input_weight
        + crafter_skill * crafting.preservation_quality_skill_weight)
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_constants() -> CraftingConstants {
        CraftingConstants {
            preservation_quality_input_weight: 0.7,
            preservation_quality_skill_weight: 0.3,
            preservation_skill_baseline: 0.4,
            ..CraftingConstants::default()
        }
    }

    #[test]
    fn perfect_input_perfect_skill_yields_perfect_output() {
        let q = preservation_output_quality(1.0, 1.0, &test_constants());
        assert!((q - 1.0).abs() < 1e-6, "got {q}");
    }

    #[test]
    fn zero_input_zero_skill_yields_zero_output() {
        let q = preservation_output_quality(0.0, 0.0, &test_constants());
        assert!(q.abs() < 1e-6, "got {q}");
    }

    #[test]
    fn default_skill_baseline_lifts_unskilled_crafters_off_floor() {
        // Default-skill crafter: herbcraft 0.05, foraging 0.1 →
        // normalised crafter_skill ≈ (0.025 + 0.03 + 0.4) = 0.455.
        // Perfect input quality → output = 0.7 + 0.455*0.3 ≈ 0.8365.
        let crafting = test_constants();
        let crafter_skill =
            (0.05 * 0.5 + 0.1 * 0.3 + crafting.preservation_skill_baseline).clamp(0.0, 1.0);
        let q = preservation_output_quality(1.0, crafter_skill, &crafting);
        assert!(
            q > 0.80 && q < 0.85,
            "default-skill crafter on perfect input: got {q}, expected ~0.83",
        );
    }

    #[test]
    fn highly_skilled_crafter_recovers_mediocre_input() {
        // Highly skilled crafter: herbcraft 1.5, foraging 1.0 →
        // normalised = clamp(0.75 + 0.3 + 0.4) = clamp(1.45) = 1.0.
        // Mediocre input 0.5 → output = 0.35 + 0.3 = 0.65.
        let crafting = test_constants();
        let crafter_skill =
            (1.5 * 0.5 + 1.0 * 0.3 + crafting.preservation_skill_baseline).clamp(0.0, 1.0);
        let q = preservation_output_quality(0.5, crafter_skill, &crafting);
        assert!(
            q > 0.60 && q < 0.70,
            "highly-skilled crafter on mediocre input: got {q}, expected ~0.65",
        );
    }

    #[test]
    fn output_is_always_clamped_to_unit_interval() {
        let c = test_constants();
        assert!((0.0..=1.0).contains(&preservation_output_quality(2.0, 2.0, &c)));
        assert!((0.0..=1.0).contains(&preservation_output_quality(-1.0, -1.0, &c)));
        assert!((0.0..=1.0).contains(&preservation_output_quality(1.0, -0.5, &c)));
    }
}
