use bevy_ecs::prelude::*;

use crate::components::magic::{Harvestable, Herb, Inventory};
use crate::components::skills::Skills;
use crate::resources::sim_constants::MagicConstants;
use crate::steps::StepResult;

/// # GOAP step resolver: `GatherHerb`
///
/// **Real-world effect** — on completion (`ticks >=
/// gather_herb_ticks`), removes a target `Herb` entity from the
/// world and adds it to the actor's `Inventory.herbs`; grows
/// herbcraft skill.
///
/// **Plan-level preconditions** — emitted by the herbcraft
/// planner; target herb is resolved at plan-build time.
///
/// **Runtime preconditions** — Fail if `target_entity` is None,
/// if the herb was already taken (race), or if inventory is
/// full. No silent-advance surface.
///
/// **Witness** — returns plain `StepResult` (predates the
/// `StepOutcome<W>` convention). Caller at
/// `src/systems/goap.rs:2220` records
/// `Feature::GatherHerbCompleted` on `Advance`; that's acceptable
/// here because every Advance path is witness-equivalent
/// (`commands.entity(herb_e).despawn()` only runs on success).
///
/// **Feature emission** — `Feature::GatherHerbCompleted`
/// (Positive) on `Advance`.
pub fn resolve_gather_herb(
    ticks: u64,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    skills: &mut Skills,
    herb_entities: &Query<
        (Entity, &Herb, &crate::components::physical::Position),
        With<Harvestable>,
    >,
    commands: &mut Commands,
    m: &MagicConstants,
) -> StepResult {
    if ticks >= m.gather_herb_ticks {
        if let Some(herb_e) = target_entity {
            if let Ok((_, herb, _)) = herb_entities.get(herb_e) {
                if inventory.add_herb(herb.kind) {
                    commands.entity(herb_e).despawn();
                    skills.herbcraft += skills.growth_rate() * m.herbcraft_gather_skill_growth;
                    StepResult::Advance
                } else {
                    StepResult::Fail("inventory full".into())
                }
            } else {
                StepResult::Fail("herb already taken".into())
            }
        } else {
            StepResult::Fail("no herb target".into())
        }
    } else {
        StepResult::Continue
    }
}
