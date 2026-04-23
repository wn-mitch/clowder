use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::magic::{RemedyEffect, RemedyKind};
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::map::TileMap;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::sim_constants::MagicConstants;
use crate::steps::StepResult;

/// # GOAP step resolver: `ApplyRemedy`
///
/// **Real-world effect** — paths the healer to the patient, then
/// applies a `RemedyEffect` component to the patient (healing,
/// corruption cleanse, etc). Grows herbcraft skill. On
/// completion, yields a deferred `(patient, healer, fondness)`
/// gratitude tuple the caller applies to the patient's
/// relationship with the healer.
///
/// **Plan-level preconditions** — emitted by herbcraft planner
/// after a successful `PrepareRemedy` step populates the
/// remedy's expected herb in inventory.
///
/// **Runtime preconditions** — `target_entity` + `target_position`
/// must be `Some`; Fail if either missing or if the patient is
/// dead (`!patient_alive`).
///
/// **Witness** — returns `(StepResult, Option<(Entity, Entity,
/// f32)>)`; the `Option` payload is the gratitude deferred-
/// effect, present iff the remedy was actually applied. This
/// predates the `StepOutcome<W>` convention but the Option-
/// witness shape maps cleanly to
/// `StepOutcome<Option<(Entity, Entity, f32)>>` — a future
/// refactor could migrate it.
///
/// **Feature emission** — caller records `Feature::RemedyApplied`
/// (Positive) when the Option payload is `Some`, which is
/// already the correctly-gated shape.
#[allow(clippy::too_many_arguments)]
pub fn resolve_apply_remedy(
    remedy: RemedyKind,
    cat_entity: Entity,
    target_position: Option<Position>,
    target_entity: Option<Entity>,
    patient_alive: bool,
    cached_path: &mut Option<Vec<Position>>,
    pos: &mut Position,
    skills: &mut Skills,
    map: &TileMap,
    commands: &mut Commands,
    log: &mut NarrativeLog,
    tick: u64,
    m: &MagicConstants,
) -> (StepResult, Option<(Entity, Entity, f32)>) {
    // Move to patient if not adjacent.
    if let Some(target_pos) = target_position {
        if pos.manhattan_distance(&target_pos) > 1 {
            if cached_path.is_none() {
                match find_path(*pos, target_pos, map) {
                    Some(path) => *cached_path = Some(path),
                    None => return (StepResult::Fail("no path to patient".into()), None),
                }
            }
            if let Some(ref mut path) = cached_path {
                if !path.is_empty() {
                    *pos = path.remove(0);
                }
            }
            return (StepResult::Continue, None);
        }
    }

    // Apply remedy to target.
    let Some(patient) = target_entity else {
        return (StepResult::Fail("no patient for remedy".into()), None);
    };
    if !patient_alive {
        return (StepResult::Fail("patient no longer alive".into()), None);
    }
    commands.entity(patient).insert(RemedyEffect {
        kind: remedy,
        ticks_remaining: remedy.duration(),
        healer: Some(cat_entity),
    });
    let gratitude = Some((patient, cat_entity, m.gratitude_fondness_gain));

    log.push(
        tick,
        "A herbalist applies a remedy with careful paws.".to_string(),
        NarrativeTier::Action,
    );
    skills.herbcraft += skills.growth_rate() * m.herbcraft_apply_skill_growth;
    (StepResult::Advance, gratitude)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::SystemState;

    fn make_commands_world() -> World {
        World::new()
    }

    #[test]
    fn apply_remedy_fails_no_path() {
        let mut world = make_commands_world();
        let mut state: SystemState<Commands> = SystemState::new(&mut world);

        // 5x5 map; surround target (4,4) with water on all 8 neighbours.
        let mut map = TileMap::new(6, 6, crate::resources::map::Terrain::Grass);
        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                map.set(4 + dx, 4 + dy, crate::resources::map::Terrain::Water);
            }
        }

        let mut log = NarrativeLog::default();
        let m = MagicConstants::default();
        let mut pos = Position::new(0, 0);
        let mut skills = Skills::default();
        let mut cached_path = None;
        let cat = world.spawn_empty().id();

        let mut commands = state.get_mut(&mut world);
        let (result, gratitude) = resolve_apply_remedy(
            RemedyKind::HealingPoultice,
            cat,
            Some(Position::new(4, 4)), // far away, unreachable
            Some(cat),                 // irrelevant — we never reach this
            true,
            &mut cached_path,
            &mut pos,
            &mut skills,
            &map,
            &mut commands,
            &mut log,
            100,
            &m,
        );
        assert!(
            matches!(result, StepResult::Fail(ref reason) if reason == "no path to patient"),
            "expected 'no path to patient' Fail, got {result:?}"
        );
        assert!(gratitude.is_none());
    }

    #[test]
    fn apply_remedy_fails_dead_patient() {
        let mut world = make_commands_world();
        let mut state: SystemState<Commands> = SystemState::new(&mut world);

        let map = TileMap::new(5, 5, crate::resources::map::Terrain::Grass);
        let mut log = NarrativeLog::default();
        let m = MagicConstants::default();
        let mut pos = Position::new(2, 2);
        let mut skills = Skills::default();
        let mut cached_path = None;
        let cat = world.spawn_empty().id();
        let patient = world.spawn_empty().id();

        let mut commands = state.get_mut(&mut world);
        let (result, gratitude) = resolve_apply_remedy(
            RemedyKind::HealingPoultice,
            cat,
            None,          // no movement needed — skip straight to apply
            Some(patient), // patient exists but is dead
            false,         // patient_alive = false
            &mut cached_path,
            &mut pos,
            &mut skills,
            &map,
            &mut commands,
            &mut log,
            100,
            &m,
        );
        assert!(
            matches!(result, StepResult::Fail(ref reason) if reason == "patient no longer alive"),
            "expected 'patient no longer alive' Fail, got {result:?}"
        );
        assert!(gratitude.is_none());
    }

    #[test]
    fn apply_remedy_fails_no_patient() {
        let mut world = make_commands_world();
        let mut state: SystemState<Commands> = SystemState::new(&mut world);

        let map = TileMap::new(5, 5, crate::resources::map::Terrain::Grass);
        let mut log = NarrativeLog::default();
        let m = MagicConstants::default();
        let mut pos = Position::new(2, 2);
        let mut skills = Skills::default();
        let mut cached_path = None;
        let cat = world.spawn_empty().id();

        let mut commands = state.get_mut(&mut world);
        let (result, gratitude) = resolve_apply_remedy(
            RemedyKind::HealingPoultice,
            cat,
            None, // no movement
            None, // no target entity at all
            false,
            &mut cached_path,
            &mut pos,
            &mut skills,
            &map,
            &mut commands,
            &mut log,
            100,
            &m,
        );
        assert!(
            matches!(result, StepResult::Fail(ref reason) if reason == "no patient for remedy"),
            "expected 'no patient for remedy' Fail, got {result:?}"
        );
        assert!(gratitude.is_none());
    }
}
