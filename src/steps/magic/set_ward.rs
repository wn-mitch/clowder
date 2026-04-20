use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::{Inventory, MisfireEffect, Ward, WardKind};
use crate::components::mental::Mood;
use crate::components::physical::{Health, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
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
    event_log: Option<&mut EventLog>,
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

        // Spawn the ward entity. Thornward decay rate is configurable.
        let mut ward = match kind {
            WardKind::Thornward => Ward::thornward(),
            WardKind::DurableWard => Ward::durable(),
        };
        if kind == WardKind::Thornward {
            ward.decay_rate = m.thornward_decay_rate;
        }
        let spawn_strength = ward.strength;
        commands.spawn((ward, Position::new(pos.x, pos.y)));
        if let Some(elog) = event_log {
            elog.push(
                tick,
                EventKind::WardPlaced {
                    cat: cat_name.to_string(),
                    ward_kind: format!("{kind:?}"),
                    location: (pos.x, pos.y),
                    strength: spawn_strength,
                },
            );
        }
        skills.herbcraft += skills.growth_rate() * m.herbcraft_ward_skill_growth;
        // Magic-affinity cats absorb magic practice from any ward work.
        // This gives gifted-but-untrained cats a natural progression path:
        // they work herbcraft wards alongside the rest of the colony, and
        // their magic skill climbs until durable wards become viable. Cats
        // without affinity gain herbcraft only, as intended.
        if magic_aff.0 > 0.2 || kind == WardKind::DurableWard {
            skills.magic += skills.growth_rate() * m.magic_ward_skill_growth;
        }
        let text = match kind {
            WardKind::Thornward => {
                let variants = [
                    format!("{cat_name} traces thornbriar sigils into the earth. A ward stands."),
                    format!("{cat_name} weaves thornbriar into a warding sigil."),
                    format!("{cat_name} presses thornbriar into the soil — the air tightens."),
                ];
                variants[rng.random_range(0..variants.len())].clone()
            }
            WardKind::DurableWard => {
                let variants = [
                    format!("{cat_name} chants the old words. A durable ward takes root."),
                    format!("{cat_name} sets a deep ward, felt more than seen."),
                ];
                variants[rng.random_range(0..variants.len())].clone()
            }
        };
        log.push(tick, text, NarrativeTier::Significant);
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
