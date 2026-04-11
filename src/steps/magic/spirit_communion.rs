use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::MisfireEffect;
use crate::components::mental::{Mood, MoodModifier};
use crate::components::physical::{Health, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::narrative::NarrativeLog;
use crate::resources::sim_constants::{CombatConstants, MagicConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::steps::StepResult;

#[allow(clippy::too_many_arguments)]
pub fn resolve_spirit_communion(
    ticks: u64,
    cat_name: &str,
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
    activation: &mut SystemActivation,
    m: &MagicConstants,
    combat: &CombatConstants,
) -> StepResult {
    if ticks == 1 {
        if let Some(misfire) =
            crate::systems::magic::check_misfire(magic_aff.0, skills.magic, rng, m)
        {
            crate::systems::magic::apply_misfire(
                misfire, cat_name, mood, corruption, health, pos, commands, log, tick, m, combat,
            );
            if matches!(misfire, MisfireEffect::Fizzle) {
                return StepResult::Fail("misfire: fizzle".into());
            }
        }
    }
    if ticks >= m.spirit_communion_ticks {
        activation.record(Feature::SpiritCommunion);
        mood.modifiers.push_back(MoodModifier {
            amount: m.spirit_communion_mood_bonus,
            ticks_remaining: m.spirit_communion_mood_ticks,
            source: "spirit communion".to_string(),
        });
        skills.magic += skills.growth_rate() * m.spirit_communion_skill_growth;
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
