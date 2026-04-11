use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::{Inventory, MisfireEffect, Ward, WardKind};
use crate::components::mental::Mood;
use crate::components::physical::{Health, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::narrative::NarrativeLog;
use crate::resources::sim_constants::{CombatConstants, MagicConstants};
use crate::steps::StepResult;

#[allow(clippy::too_many_arguments)]
pub fn resolve_set_ward(
    ticks: u64,
    kind: WardKind,
    cat_name: &str,
    inventory: &mut Inventory,
    magic_aff: &MagicAffinity,
    skills: &mut Skills,
    mood: &mut Mood,
    corruption: &mut Corruption,
    health: &mut Health,
    pos: &Position,
    rng: &mut impl Rng,
    commands: &mut Commands,
    log: &mut NarrativeLog,
    tick: u64,
    m: &MagicConstants,
    combat: &CombatConstants,
) -> StepResult {
    if ticks >= m.set_ward_ticks {
        // Consume thornbriar if setting a thornward.
        if kind == WardKind::Thornward
            && !inventory.take_herb(crate::components::magic::HerbKind::Thornbriar)
        {
            return StepResult::Fail("no thornbriar for ward".into());
        }

        // Check for misfire on magical actions.
        if kind == WardKind::DurableWard {
            if let Some(misfire) =
                crate::systems::magic::check_misfire(magic_aff.0, skills.magic, rng, m)
            {
                crate::systems::magic::apply_misfire(
                    misfire, cat_name, mood, corruption, health, pos, commands, log, tick, m,
                    combat,
                );
                if matches!(misfire, MisfireEffect::Fizzle) {
                    return StepResult::Fail("misfire: fizzle".into());
                }
                if matches!(misfire, MisfireEffect::InvertedWard) {
                    // Spawn inverted ward instead.
                    commands.spawn((Ward::inverted_at(kind), Position::new(pos.x, pos.y)));
                    return StepResult::Advance;
                }
            }
        }

        // Spawn the ward entity.
        let ward = match kind {
            WardKind::Thornward => Ward::thornward(),
            WardKind::DurableWard => Ward::durable(),
        };
        commands.spawn((ward, Position::new(pos.x, pos.y)));
        skills.herbcraft += skills.growth_rate() * m.herbcraft_ward_skill_growth;
        if kind == WardKind::DurableWard {
            skills.magic += skills.growth_rate() * m.magic_ward_skill_growth;
        }
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
