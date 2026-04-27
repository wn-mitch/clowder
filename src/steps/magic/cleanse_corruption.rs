use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::MisfireEffect;
use crate::components::mental::Mood;
use crate::components::physical::{Health, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;
use crate::resources::sim_constants::{CombatConstants, MagicConstants};
use crate::resources::time::TimeScale;
use crate::steps::StepResult;

/// # GOAP step resolver: `CleanseCorruption`
///
/// **Real-world effect** — on completion, reduces the corruption
/// tile value at the actor's position by a skill-scaled amount
/// and grows magic skill. Includes a misfire roll on first tick.
///
/// **Plan-level preconditions** — emitted by the magic planner
/// for cleanse DSEs.
///
/// **Runtime preconditions** — none; the tile-mutation path
/// runs on the Advance branch unconditionally.
///
/// **Witness** — returns plain `StepResult`. Success is implicit
/// on Advance (corruption mutation runs unconditionally at that
/// branch).
///
/// **Feature emission** — caller records `Feature::CleanseCompleted`
/// (Positive) on Advance.
#[allow(clippy::too_many_arguments)]
pub fn resolve_cleanse_corruption(
    ticks: u64,
    cat_name: &str,
    magic_aff: &MagicAffinity,
    skills: &mut Skills,
    corruption: &mut Corruption,
    mood: &mut Mood,
    health: &mut Health,
    pos: &Position,
    map: &mut TileMap,
    rng: &mut impl Rng,
    commands: &mut Commands,
    log: &mut NarrativeLog,
    tick: u64,
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
            );
            if matches!(misfire, MisfireEffect::Fizzle) {
                return StepResult::Fail("misfire: fizzle".into());
            }
        }
    }

    let cleanse_corruption_rate = m.cleanse_corruption_rate.per_tick(time_scale);
    let cleanse_personal_corruption_rate = m.cleanse_personal_corruption_rate.per_tick(time_scale);

    // Per-tick: reduce tile corruption.
    if map.in_bounds(pos.x, pos.y) {
        let tile = map.get_mut(pos.x, pos.y);
        tile.corruption = (tile.corruption - skills.magic * cleanse_corruption_rate).max(0.0);
    }
    // Occupational hazard: personal corruption increases.
    corruption.0 = (corruption.0 + cleanse_personal_corruption_rate).min(1.0);
    skills.magic += skills.growth_rate() * m.cleanse_magic_skill_growth;

    // Advance when tile is cleansed or after max ticks.
    let done = if map.in_bounds(pos.x, pos.y) {
        map.get(pos.x, pos.y).corruption < m.cleanse_done_threshold
    } else {
        true
    };
    if done || ticks >= m.cleanse_max_ticks {
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
