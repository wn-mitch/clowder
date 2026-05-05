use bevy_ecs::prelude::*;

use crate::components::identity::{Age, LifeStage, Species};
use crate::components::kitten::KittenDependency;
use crate::components::markers;
use crate::components::mental::{Mood, MoodModifier, MoodSource};
use crate::components::physical::{Dead, Needs, Position};
use crate::resources::colony_score::ColonyScore;
use crate::resources::sim_constants::SimConstants;
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
///
/// **Ticket 166** — at the maturation transition, increments
/// `ColonyScore.kittens_surviving`. The `BornInSim` marker added at the
/// kitten-spawn site (see `pregnancy.rs`) survives maturation, so the
/// matching decrement in `death.rs::check_death` can identify
/// in-sim-born matured adults at death-time.
pub fn tick_kitten_growth(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut query: Query<(Entity, &mut KittenDependency), Without<Dead>>,
    mut commands: Commands,
    mut activation: Option<ResMut<SystemActivation>>,
    mut colony_score: Option<ResMut<ColonyScore>>,
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
            if let Some(ref mut score) = colony_score {
                score.kittens_surviving += 1;
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
            mood.modifiers.push_back(
                MoodModifier::new(total, 2, "kitten_aura").with_kind(MoodSource::Social),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// update_kitten_cry_map (ticket 006 — §5.6.3 row #13;
// repurposed by ticket 156; ticket 161 merged the
// IsParentOfHungryKitten author here to avoid adding a new schedule
// conflict edge to Bevy's parallel scheduler — see ticket file
// `docs/open-work/tickets/161-…md` for the cascade analysis)
// ---------------------------------------------------------------------------

/// Re-stamp `KittenCryMap` and author the `IsParentOfHungryKitten`
/// marker from live kittens whose hunger has fallen below
/// `kitten_cry_hunger_threshold`. §5.6.3 row #13 — repurposed from
/// sight × colony to hearing × colony by ticket 156.
///
/// Each crying kitten paints a linear-falloff disc of
/// `kitten_cry_sense_range` tiles, strength `(threshold - hunger) /
/// threshold` so a quiet kitten doesn't paint and a starving kitten
/// paints loudly. Adults near multiple crying kittens see the
/// contributions sum (clamped to 1.0). Re-stamped per tick rather than
/// decayed because kittens move and hunger changes fast.
///
/// **Ticket 161 merge** — the `IsParentOfHungryKitten` author was
/// previously a separate Chain 2a system. Both subsystems read
/// `&Needs` on kittens with the same predicate (hunger below
/// `kitten_cry_hunger_threshold`), so co-locating them avoids adding
/// a *new* schedule conflict edge between an `&Needs` reader and
/// every `&mut Needs` writer in the schedule. Adding such an edge in
/// the post-158 build re-ordered Bevy's topological sort enough to
/// flip a movement tie-break at tick 1201300 of the seed-42 soak,
/// cascading into a 6-cat fox-attrition extinction window.
///
/// **§4.3 ordering hazard.** When a kitten dies, the surviving
/// parent's marker is removed within the same tick (the kitten's
/// `KittenDependency` stops counting once `With<Dead>` filters it
/// out). Don't infer parent-at-death status from this marker on the
/// death tick — the canonical channel is the future
/// `CatDied.survivors_by_relationship` payload.
#[allow(clippy::type_complexity)]
pub fn update_kitten_cry_map(
    mut commands: Commands,
    kittens: Query<(&Position, &Needs, &KittenDependency), Without<Dead>>,
    cats: Query<
        (Entity, Has<markers::IsParentOfHungryKitten>),
        (With<Species>, Without<Dead>),
    >,
    mut map: ResMut<crate::resources::KittenCryMap>,
    constants: Res<SimConstants>,
) {
    use std::collections::HashSet;
    let sense_range = constants.influence_maps.kitten_cry_sense_range;
    let threshold = constants.influence_maps.kitten_cry_hunger_threshold;
    map.clear();

    let mut parents_with_hungry_kitten: HashSet<Entity> = HashSet::new();

    if threshold > 0.0 {
        for (pos, needs, dep) in &kittens {
            if needs.hunger >= threshold {
                continue;
            }
            let strength = ((threshold - needs.hunger) / threshold).clamp(0.0, 1.0);
            if strength > 0.0 {
                map.stamp(pos.x, pos.y, strength, sense_range);
            }
            if let Some(m) = dep.mother {
                parents_with_hungry_kitten.insert(m);
            }
            if let Some(f) = dep.father {
                parents_with_hungry_kitten.insert(f);
            }
        }
    }

    for (entity, has_marker) in &cats {
        let want = parents_with_hungry_kitten.contains(&entity);
        match (want, has_marker) {
            (true, false) => {
                commands
                    .entity(entity)
                    .insert(markers::IsParentOfHungryKitten);
            }
            (false, true) => {
                commands
                    .entity(entity)
                    .remove::<markers::IsParentOfHungryKitten>();
            }
            _ => {}
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
                commands.entity(entity).insert(markers::Kitten).remove::<(
                    markers::Young,
                    markers::Adult,
                    markers::Elder,
                )>();
            }
            LifeStage::Young if !has_y => {
                commands.entity(entity).insert(markers::Young).remove::<(
                    markers::Kitten,
                    markers::Adult,
                    markers::Elder,
                )>();
            }
            LifeStage::Adult if !has_a => {
                commands.entity(entity).insert(markers::Adult).remove::<(
                    markers::Kitten,
                    markers::Young,
                    markers::Elder,
                )>();
            }
            LifeStage::Elder if !has_e => {
                commands.entity(entity).insert(markers::Elder).remove::<(
                    markers::Kitten,
                    markers::Young,
                    markers::Adult,
                )>();
            }
            _ => {} // already has the correct marker — no-op
        }
    }
}

// ---------------------------------------------------------------------------
// update_parent_markers (Ticket 014 §4 Reproduction marker)
// ---------------------------------------------------------------------------

/// Author the `Parent` ZST on every living cat that has at least one
/// living dependent kitten with `mother == self` or `father == self`.
///
/// **Predicate** — `Parent` iff `∃ living KittenDependency d : d.mother == self ∨ d.father == self`.
/// First authoring of this marker; no inline predicate is being
/// retired. The marker is staged for future grief / aspiration
/// consumers — there is no DSE `.require()` cutover today.
///
/// **§4.3 ordering hazard.** Grief consumers MUST NOT infer
/// parent-at-time-of-death status from `With<Parent>` on a survivor
/// post-death. When a kitten dies, the surviving parent's `Parent`
/// marker is removed within the same tick (the kitten's
/// `KittenDependency` stops counting once `With<Dead>` filters it
/// out, then `cleanup_dead` despawns it). A bereaved-parent grief
/// emitter that queries `With<Parent>` after the death cleanup
/// would see a false negative for parents whose only kitten just
/// died. The canonical parent-at-time-of-death channel is the
/// future `CatDied.survivors_by_relationship` event payload — see
/// `docs/systems/ai-substrate-refactor.md` §4.3 prose.
///
/// **Ordering** — Chain 2a, before the GOAP / disposition scoring
/// loops so the snapshot population sees the freshly-authored marker.
/// Sibling of `update_life_stage_markers` in growth.rs.
#[allow(clippy::type_complexity)]
pub fn update_parent_markers(
    mut commands: Commands,
    kittens: Query<&KittenDependency, Without<Dead>>,
    cats: Query<
        (Entity, Has<markers::Parent>),
        (With<Species>, Without<Dead>),
    >,
) {
    use std::collections::HashSet;
    let mut parents: HashSet<Entity> = HashSet::new();
    for dep in kittens.iter() {
        if let Some(m) = dep.mother {
            parents.insert(m);
        }
        if let Some(f) = dep.father {
            parents.insert(f);
        }
    }
    for (entity, has_marker) in cats.iter() {
        let want = parents.contains(&entity);
        match (want, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(markers::Parent);
            }
            (false, true) => {
                commands.entity(entity).remove::<markers::Parent>();
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

    // -----------------------------------------------------------------------
    // §4 Parent marker — author tests
    // -----------------------------------------------------------------------

    use crate::components::physical::DeathCause;

    fn setup_parent() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_parent_markers);
        (world, schedule)
    }

    fn spawn_adult(world: &mut World) -> Entity {
        world.spawn(Species).id()
    }

    fn spawn_kitten(world: &mut World, mother: Entity, father: Entity) -> Entity {
        world
            .spawn((Species, KittenDependency::new(mother, father)))
            .id()
    }

    #[test]
    fn solo_cat_no_parent() {
        let (mut world, mut schedule) = setup_parent();
        let cat = spawn_adult(&mut world);
        schedule.run(&mut world);
        assert!(!world.entity(cat).contains::<markers::Parent>());
    }

    #[test]
    fn mother_with_living_kitten_gets_parent() {
        let (mut world, mut schedule) = setup_parent();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let _kitten = spawn_kitten(&mut world, mother, father);
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
        assert!(world.entity(father).contains::<markers::Parent>());
    }

    #[test]
    fn matured_kitten_drops_parent_marker() {
        let (mut world, mut schedule) = setup_parent();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let kitten = spawn_kitten(&mut world, mother, father);
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
        // Maturation in `tick_kitten_growth` removes KittenDependency.
        // Simulate by removing it directly here.
        world.entity_mut(kitten).remove::<KittenDependency>();
        schedule.run(&mut world);
        assert!(!world.entity(mother).contains::<markers::Parent>());
        assert!(!world.entity(father).contains::<markers::Parent>());
    }

    #[test]
    fn dead_kitten_excluded_so_parent_drops() {
        let (mut world, mut schedule) = setup_parent();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let kitten = spawn_kitten(&mut world, mother, father);
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
        // Kill the kitten — the §4.3 ordering hazard says the parent's
        // marker should drop within the same tick (the canonical
        // parent-at-time-of-death channel is the future
        // CatDied.survivors_by_relationship event payload).
        world.entity_mut(kitten).insert(Dead {
            tick: 0,
            cause: DeathCause::Starvation,
        });
        schedule.run(&mut world);
        assert!(!world.entity(mother).contains::<markers::Parent>());
        assert!(!world.entity(father).contains::<markers::Parent>());
    }

    #[test]
    fn dead_parent_no_marker_authoring() {
        let (mut world, mut schedule) = setup_parent();
        let father = spawn_adult(&mut world);
        // Mother is dead at the time of the author tick.
        let mother = world
            .spawn((
                Species,
                Dead {
                    tick: 0,
                    cause: DeathCause::Starvation,
                },
            ))
            .id();
        let _kitten = spawn_kitten(&mut world, mother, father);
        schedule.run(&mut world);
        // Father is living and has the kitten → Parent.
        assert!(world.entity(father).contains::<markers::Parent>());
        // Dead mother is filtered out of the cats query → no marker.
        assert!(!world.entity(mother).contains::<markers::Parent>());
    }

    #[test]
    fn parent_marker_idempotent() {
        let (mut world, mut schedule) = setup_parent();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let _kitten = spawn_kitten(&mut world, mother, father);
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
    }

    #[test]
    fn parent_marker_aggregates_multiple_kittens() {
        let (mut world, mut schedule) = setup_parent();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let kitten_a = spawn_kitten(&mut world, mother, father);
        let _kitten_b = spawn_kitten(&mut world, mother, father);
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
        // Drop one kitten — parent stays because the other is alive.
        world.entity_mut(kitten_a).remove::<KittenDependency>();
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
    }

    #[test]
    fn parent_marker_handles_unknown_father() {
        // KittenDependency is `Option<Entity>` for both parents — the
        // father field can be None (e.g. unknown sire). Mother-only
        // kittens still mark the mother.
        let (mut world, mut schedule) = setup_parent();
        let mother = spawn_adult(&mut world);
        let _kitten = world
            .spawn((
                Species,
                KittenDependency {
                    mother: Some(mother),
                    father: None,
                    maturity: 0.0,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(world.entity(mother).contains::<markers::Parent>());
    }

    // -----------------------------------------------------------------------
    // Ticket 158 — IsParentOfHungryKitten marker tests
    // -----------------------------------------------------------------------

    fn setup_hungry_marker() -> (World, Schedule) {
        let mut world = World::new();
        // Ticket 161 — the marker author was merged into
        // `update_kitten_cry_map`, which additionally requires a
        // `KittenCryMap` resource. Default `SimConstants` put the
        // threshold at 0.5; tests rely on that default.
        world.insert_resource(SimConstants::default());
        world.insert_resource(crate::resources::KittenCryMap::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(update_kitten_cry_map);
        (world, schedule)
    }

    fn spawn_kitten_with_hunger(
        world: &mut World,
        mother: Entity,
        father: Entity,
        hunger: f32,
    ) -> Entity {
        use crate::components::physical::Needs;
        // Ticket 161 — `update_kitten_cry_map`'s kittens query reads
        // `&Position`. Tests don't care which tile (the marker
        // authoring is position-independent), but the component must
        // exist for the entity to match the query.
        world
            .spawn((
                Species,
                KittenDependency::new(mother, father),
                Needs {
                    hunger,
                    ..Needs::default()
                },
                Position { x: 0, y: 0 },
            ))
            .id()
    }

    #[test]
    fn hungry_kitten_marks_both_parents() {
        let (mut world, mut schedule) = setup_hungry_marker();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let _kitten = spawn_kitten_with_hunger(&mut world, mother, father, 0.2);
        schedule.run(&mut world);
        assert!(
            world
                .entity(mother)
                .contains::<markers::IsParentOfHungryKitten>(),
            "mother should be marked when kitten hunger is below threshold"
        );
        assert!(
            world
                .entity(father)
                .contains::<markers::IsParentOfHungryKitten>(),
            "father should be marked when kitten hunger is below threshold"
        );
    }

    #[test]
    fn well_fed_kitten_does_not_mark_parents() {
        let (mut world, mut schedule) = setup_hungry_marker();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        // Hunger 0.8 is above the default 0.5 threshold.
        let _kitten = spawn_kitten_with_hunger(&mut world, mother, father, 0.8);
        schedule.run(&mut world);
        assert!(!world
            .entity(mother)
            .contains::<markers::IsParentOfHungryKitten>());
        assert!(!world
            .entity(father)
            .contains::<markers::IsParentOfHungryKitten>());
    }

    #[test]
    fn marker_clears_when_kitten_recovers() {
        let (mut world, mut schedule) = setup_hungry_marker();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let kitten = spawn_kitten_with_hunger(&mut world, mother, father, 0.1);
        schedule.run(&mut world);
        assert!(world
            .entity(mother)
            .contains::<markers::IsParentOfHungryKitten>());
        // Feed the kitten — hunger jumps above threshold.
        use crate::components::physical::Needs;
        world.entity_mut(kitten).insert(Needs {
            hunger: 0.9,
            ..Needs::default()
        });
        schedule.run(&mut world);
        assert!(
            !world
                .entity(mother)
                .contains::<markers::IsParentOfHungryKitten>(),
            "marker should clear once kitten hunger rises above threshold"
        );
    }

    #[test]
    fn dead_kitten_clears_marker() {
        let (mut world, mut schedule) = setup_hungry_marker();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let kitten = spawn_kitten_with_hunger(&mut world, mother, father, 0.1);
        schedule.run(&mut world);
        assert!(world
            .entity(mother)
            .contains::<markers::IsParentOfHungryKitten>());
        // Kitten dies — the `Without<Dead>` filter on the kittens
        // query excludes it, so the marker drops within the same tick.
        // Same §4.3 ordering hazard as `update_parent_markers`.
        world.entity_mut(kitten).insert(Dead {
            tick: 0,
            cause: DeathCause::Starvation,
        });
        schedule.run(&mut world);
        assert!(!world
            .entity(mother)
            .contains::<markers::IsParentOfHungryKitten>());
        assert!(!world
            .entity(father)
            .contains::<markers::IsParentOfHungryKitten>());
    }

    #[test]
    fn one_hungry_kitten_among_siblings_keeps_marker() {
        // Mother has two kittens — one well-fed, one starving. The marker
        // fires on ANY hungry dependent.
        let (mut world, mut schedule) = setup_hungry_marker();
        let mother = spawn_adult(&mut world);
        let father = spawn_adult(&mut world);
        let _well_fed = spawn_kitten_with_hunger(&mut world, mother, father, 0.9);
        let _hungry = spawn_kitten_with_hunger(&mut world, mother, father, 0.2);
        schedule.run(&mut world);
        assert!(world
            .entity(mother)
            .contains::<markers::IsParentOfHungryKitten>());
        assert!(world
            .entity(father)
            .contains::<markers::IsParentOfHungryKitten>());
    }

    // -----------------------------------------------------------------------
    // Ticket 166 — kittens_surviving increment on maturation
    // -----------------------------------------------------------------------

    fn setup_growth() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            ..Default::default()
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(ColonyScore::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(tick_kitten_growth);
        (world, schedule)
    }

    #[test]
    fn maturation_increments_kittens_surviving() {
        let (mut world, mut schedule) = setup_growth();
        // Spawn a kitten one tick away from maturation. ticks_per_season
        // default = 20_000, so rate = 1.0 / 80_000. Setting maturity to
        // (1.0 - rate) makes the next tick cross the threshold.
        let rate = 1.0 / (4.0 * SimConfig::default().ticks_per_season as f32);
        let kitten = world
            .spawn(KittenDependency {
                mother: None,
                father: None,
                maturity: 1.0 - rate * 0.5,
            })
            .id();

        schedule.run(&mut world);

        assert_eq!(
            world.resource::<ColonyScore>().kittens_surviving,
            1,
            "maturation should increment kittens_surviving"
        );
        assert!(
            !world.entity(kitten).contains::<KittenDependency>(),
            "matured kitten should have KittenDependency removed"
        );
    }

    #[test]
    fn pre_maturation_tick_does_not_increment() {
        let (mut world, mut schedule) = setup_growth();
        // Fresh kitten, far from maturation.
        let _kitten = world
            .spawn(KittenDependency {
                mother: None,
                father: None,
                maturity: 0.0,
            })
            .id();

        schedule.run(&mut world);

        assert_eq!(
            world.resource::<ColonyScore>().kittens_surviving,
            0,
            "non-maturing tick must not increment"
        );
    }

    #[test]
    fn maturation_idempotent_after_dependency_removed() {
        // Once KittenDependency is gone, the cat no longer matches the
        // query, so further ticks do not re-increment.
        let (mut world, mut schedule) = setup_growth();
        let rate = 1.0 / (4.0 * SimConfig::default().ticks_per_season as f32);
        let _kitten = world
            .spawn(KittenDependency {
                mother: None,
                father: None,
                maturity: 1.0 - rate * 0.5,
            })
            .id();

        schedule.run(&mut world);
        schedule.run(&mut world);
        schedule.run(&mut world);

        assert_eq!(
            world.resource::<ColonyScore>().kittens_surviving,
            1,
            "maturation should increment exactly once per kitten"
        );
    }
}
