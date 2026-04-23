use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::MisfireEffect;
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood};
use crate::components::physical::{Health, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;
use crate::resources::sim_constants::{CombatConstants, MagicConstants};
use crate::steps::StepResult;

/// # GOAP step resolver: `Scry`
///
/// **Real-world effect** — on first tick, rolls a misfire check;
/// on completion (`ticks >= scry_ticks`), adds a
/// `MemoryEntry { event_type: ResourceFound, location: random
/// tile }` to the actor's memory and grows magic skill. A
/// `MisfireEffect::Fizzle` causes `Fail`.
///
/// **Plan-level preconditions** — emitted by the magic planner
/// for scrying DSEs.
///
/// **Runtime preconditions** — none beyond the misfire roll.
///
/// **Witness** — returns plain `StepResult`. Predates the
/// `StepOutcome<W>` convention; success is implicit on Advance
/// (memory insertion runs unconditionally on that path).
///
/// **Feature emission** — caller records
/// `Feature::ScryCompleted` (Positive) on Advance at
/// `src/systems/goap.rs:2377`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_scry(
    ticks: u64,
    cat_name: &str,
    magic_aff: &MagicAffinity,
    skills: &mut Skills,
    memory: &mut Memory,
    mood: &mut Mood,
    corruption: &mut Corruption,
    health: &mut Health,
    pos: &Position,
    map: &TileMap,
    rng: &mut impl Rng,
    commands: &mut Commands,
    log: &mut NarrativeLog,
    tick: u64,
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
    if ticks >= m.scry_ticks {
        let rx = rng.random_range(0..map.width);
        let ry = rng.random_range(0..map.height);
        memory.remember(MemoryEntry {
            event_type: MemoryType::ResourceFound,
            location: Some(Position::new(rx, ry)),
            involved: vec![],
            tick,
            strength: m.scry_memory_strength,
            firsthand: true,
        });
        skills.magic += skills.growth_rate() * m.scry_magic_skill_growth;
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
