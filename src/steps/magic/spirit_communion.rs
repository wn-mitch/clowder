use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::MisfireEffect;
use crate::components::mental::{Mood, MoodModifier};
use crate::components::physical::{Health, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::narrative::NarrativeLog;
use crate::resources::sim_constants::{CombatConstants, MagicConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::TimeScale;
use crate::steps::StepResult;

/// # GOAP step resolver: `SpiritCommunion`
///
/// **Real-world effect** — on completion, applies a positive
/// `MoodModifier` to the actor, grows magic skill, and records a
/// `SpiritCommunion` feature directly via `SystemActivation`
/// (unusual — most resolvers defer Feature emission to the
/// caller).
///
/// **Plan-level preconditions** — emitted by the magic planner
/// for spirit communion DSEs.
///
/// **Runtime preconditions** — misfire roll on first tick; Fail
/// on fizzle.
///
/// **Witness** — returns plain `StepResult`. The Feature
/// emission is inline in the resolver rather than caller-side.
///
/// **Feature emission** — `Feature::SpiritCommunion` (Positive),
/// recorded inline in the resolver on the Advance path.
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
    time_scale: &TimeScale,
) -> StepResult {
    if ticks == 1 {
        if let Some(misfire) =
            crate::systems::magic::check_misfire(magic_aff.0, skills.magic, rng, m)
        {
            crate::systems::magic::apply_misfire(
                misfire, cat_name, mood, corruption, health, pos, commands, log, tick, m, combat,
                time_scale,
            );
            if matches!(misfire, MisfireEffect::Fizzle) {
                return StepResult::Fail("misfire: fizzle".into());
            }
        }
    }
    if ticks >= m.spirit_communion_duration.ticks(time_scale) {
        activation.record(Feature::SpiritCommunion);
        mood.modifiers.push_back(MoodModifier {
            amount: m.spirit_communion_mood_bonus,
            ticks_remaining: m.spirit_communion_mood_duration.ticks(time_scale),
            source: "spirit communion".to_string(),
        });
        skills.magic += skills.growth_rate() * m.spirit_communion_skill_growth;
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
