use bevy_ecs::prelude::*;

use crate::components::identity::{Age, LifeStage, Species};
use crate::components::kitten::KittenDependency;
use crate::components::markers;
use crate::components::mental::{Mood, MoodModifier};
use crate::components::physical::{Dead, Position};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};

// ---------------------------------------------------------------------------
// tick_kitten_growth system
// ---------------------------------------------------------------------------

/// Advance kitten maturity each tick. At maturity >= 1.0 the
/// `KittenDependency` component is removed and the cat gains full
/// capabilities.
///
/// Maturity rate: `1.0 / (4.0 * ticks_per_season)` per tick — independence
/// after exactly 4 seasons.
pub fn tick_kitten_growth(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut query: Query<(Entity, &mut KittenDependency), Without<Dead>>,
    mut commands: Commands,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let _ = time; // reserved for future use (e.g. nutrition-based growth rate)
    let rate = 1.0 / (4.0 * config.ticks_per_season as f32);

    for (entity, mut dep) in &mut query {
        dep.maturity = (dep.maturity + rate).min(1.0);

        if dep.maturity >= 1.0 {
            commands.entity(entity).remove::<KittenDependency>();
            if let Some(ref mut act) = activation {
                act.record(Feature::KittenMatured);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// kitten_mood_aura system
// ---------------------------------------------------------------------------

/// Kittens provide a persistent mood bonus to nearby adults that scales
/// inversely with maturity. Multiple kittens stack diminishingly.
#[allow(clippy::type_complexity)]
pub fn kitten_mood_aura(
    kittens: Query<(&KittenDependency, &Position), Without<Dead>>,
    mut adults: Query<
        (&Position, &mut Mood),
        (With<Species>, Without<Dead>, Without<KittenDependency>),
    >,
) {
    let kitten_data: Vec<(f32, Position)> = kittens
        .iter()
        .map(|(dep, pos)| (dep.maturity, *pos))
        .collect();

    if kitten_data.is_empty() {
        return;
    }

    for (adult_pos, mut mood) in &mut adults {
        // Collect bonuses from nearby kittens.
        let mut bonuses: Vec<f32> = kitten_data
            .iter()
            .filter(|(_, kpos)| adult_pos.manhattan_distance(kpos) <= 5)
            .map(|(maturity, _)| 0.15 * (1.0 - maturity))
            .filter(|b| *b > 0.0)
            .collect();

        if bonuses.is_empty() {
            continue;
        }

        // Sort descending and stack diminishingly.
        bonuses.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let total: f32 = bonuses
            .iter()
            .enumerate()
            .map(|(i, b)| b * (0.5_f32).powi(i as i32))
            .sum();

        // Refresh the kitten-aura modifier each tick.
        if let Some(existing) = mood
            .modifiers
            .iter_mut()
            .find(|m| m.source == "kitten_aura")
        {
            existing.amount = total;
            existing.ticks_remaining = 2;
        } else {
            mood.modifiers.push_back(MoodModifier {
                amount: total,
                ticks_remaining: 2,
                source: "kitten_aura".to_string(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// update_life_stage_markers system (§4.3 LifeStage)
// ---------------------------------------------------------------------------

/// Maintain exactly one of {`Kitten`, `Young`, `Adult`, `Elder`} on each
/// living cat. The `Has<M>` booleans short-circuit: on steady-state ticks
/// where no cat transitions, the system iterates but issues zero commands.
///
/// Runs in Chain 2, after `update_incapacitation` and before the scoring
/// systems, so the `MarkerSnapshot` population in `evaluate_dispositions`
/// and `evaluate_and_plan` sees the freshly-authored ZSTs.
#[allow(clippy::type_complexity)]
pub fn update_life_stage_markers(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            &Age,
            Has<markers::Kitten>,
            Has<markers::Young>,
            Has<markers::Adult>,
            Has<markers::Elder>,
        ),
        Without<Dead>,
    >,
    time: Res<TimeState>,
    config: Res<SimConfig>,
) {
    for (entity, age, has_k, has_y, has_a, has_e) in &cats {
        let stage = age.stage(time.tick, config.ticks_per_season);
        match stage {
            LifeStage::Kitten if !has_k => {
                commands
                    .entity(entity)
                    .insert(markers::Kitten)
                    .remove::<(markers::Young, markers::Adult, markers::Elder)>();
            }
            LifeStage::Young if !has_y => {
                commands
                    .entity(entity)
                    .insert(markers::Young)
                    .remove::<(markers::Kitten, markers::Adult, markers::Elder)>();
            }
            LifeStage::Adult if !has_a => {
                commands
                    .entity(entity)
                    .insert(markers::Adult)
                    .remove::<(markers::Kitten, markers::Young, markers::Elder)>();
            }
            LifeStage::Elder if !has_e => {
                commands
                    .entity(entity)
                    .insert(markers::Elder)
                    .remove::<(markers::Kitten, markers::Young, markers::Adult)>();
            }
            _ => {} // already has the correct marker — no-op
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::*;

    /// Build a minimal world with TimeState + SimConfig, returning the
    /// world and a schedule containing only `update_life_stage_markers`.
    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            ..Default::default()
        });
        world.insert_resource(SimConfig::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(update_life_stage_markers);
        (world, schedule)
    }

    fn spawn_cat(world: &mut World, born_tick: u64) -> Entity {
        world.spawn(Age { born_tick }).id()
    }

    fn has_stage(world: &World, entity: Entity) -> (bool, bool, bool, bool) {
        (
            world.entity(entity).contains::<markers::Kitten>(),
            world.entity(entity).contains::<markers::Young>(),
            world.entity(entity).contains::<markers::Adult>(),
            world.entity(entity).contains::<markers::Elder>(),
        )
    }

    fn exactly_one(stage: (bool, bool, bool, bool)) -> bool {
        [stage.0, stage.1, stage.2, stage.3]
            .iter()
            .filter(|&&b| b)
            .count()
            == 1
    }

    #[test]
    fn newborn_gets_kitten_marker() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 0);
        schedule.run(&mut world);
        let stage = has_stage(&world, cat);
        assert!(stage.0, "expected Kitten marker");
        assert!(exactly_one(stage));
    }

    #[test]
    fn transitions_kitten_to_young() {
        let (mut world, mut schedule) = setup();
        // Born at tick 0, ticks_per_season = 20000 (default).
        // Young starts at season 4 → tick 80_000.
        let cat = spawn_cat(&mut world, 0);
        schedule.run(&mut world);
        assert!(has_stage(&world, cat).0, "should start as Kitten");

        world.resource_mut::<TimeState>().tick = 80_000;
        schedule.run(&mut world);
        let stage = has_stage(&world, cat);
        assert!(stage.1, "expected Young marker at tick 80000");
        assert!(exactly_one(stage));
    }

    #[test]
    fn transitions_young_to_adult() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 0);
        // Adult starts at season 12 → tick 240_000.
        world.resource_mut::<TimeState>().tick = 240_000;
        schedule.run(&mut world);
        let stage = has_stage(&world, cat);
        assert!(stage.2, "expected Adult marker at tick 240000");
        assert!(exactly_one(stage));
    }

    #[test]
    fn transitions_adult_to_elder() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 0);
        // Elder starts at season 60 → tick 1_200_000.
        world.resource_mut::<TimeState>().tick = 1_200_000;
        schedule.run(&mut world);
        let stage = has_stage(&world, cat);
        assert!(stage.3, "expected Elder marker at tick 1200000");
        assert!(exactly_one(stage));
    }

    #[test]
    fn dead_cats_are_skipped() {
        let (mut world, mut schedule) = setup();
        let cat = world
            .spawn((
                Age { born_tick: 0 },
                Dead {
                    tick: 0,
                    cause: crate::components::physical::DeathCause::Starvation,
                },
            ))
            .id();
        schedule.run(&mut world);
        let stage = has_stage(&world, cat);
        assert!(!stage.0 && !stage.1 && !stage.2 && !stage.3);
    }

    #[test]
    fn idempotent_across_ticks() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 0);
        schedule.run(&mut world);
        assert!(has_stage(&world, cat).0);
        // Run again at the same tick — should not panic or duplicate.
        schedule.run(&mut world);
        assert!(has_stage(&world, cat).0);
        assert!(exactly_one(has_stage(&world, cat)));
    }

    #[test]
    fn multiple_cats_independent() {
        let (mut world, mut schedule) = setup();
        let kitten = spawn_cat(&mut world, 0);
        let adult_born = spawn_cat(&mut world, 0);
        world.resource_mut::<TimeState>().tick = 240_000;
        schedule.run(&mut world);

        let kitten_stage = has_stage(&world, kitten);
        let adult_stage = has_stage(&world, adult_born);
        // Both born at 0, current tick 240000 → season 12 → Adult.
        assert!(kitten_stage.2, "first cat should be Adult");
        assert!(adult_stage.2, "second cat should be Adult");

        // Spawn a fresh kitten at tick 240000.
        let new_kitten = spawn_cat(&mut world, 240_000);
        schedule.run(&mut world);
        assert!(
            has_stage(&world, new_kitten).0,
            "new kitten should be Kitten"
        );
        assert!(
            has_stage(&world, adult_born).2,
            "adult should still be Adult"
        );
    }
}
