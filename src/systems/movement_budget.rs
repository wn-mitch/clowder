//! Movement-component lifecycle authors: spawn observers + save-load
//! lazy-insert (ticket 138; reshaped by 140 steps 6–13).
//!
//! See `crate::components::movement_budget` for the component-level
//! contract (`MovementBudget` is the per-entity speed cap since 140
//! step 13 — the accumulator/`try_spend_step` machinery is deleted).
//! This module:
//!
//! 1. **`insert_missing_movement_components`** — save-load compat
//!    pass: entities deserialized from saves that pre-date
//!    `MovementBudget` (pre-138) or `Velocity`/`DesiredVelocity`
//!    (pre-140) gain them here. Runs per-tick but the `Without<…>`
//!    queries are archetype-pruned to empty on every tick after the
//!    first, so the steady-state cost is nil (unlike the retired
//!    `accumulate_movement_budget`, which mutated every mover's
//!    budget every tick). Live spawns never hit this path — the
//!    observers below and `cat_bundle` author at spawn time.
//!
//! 2. **`on_wild_animal_added`** — `OnAdd<WildAnimal>` observer that
//!    inserts the species-default `MovementBudget` + the
//!    fluid-movement pair whenever a `WildAnimal` is first added.
//!    Keeps the ~20 `WildAnimal::new` call-sites untouched — one
//!    canonical author for newly-spawned wildlife (substrate-stubs
//!    discipline: one writer per component). Cats are authored
//!    separately in `cat_bundle` (`src/plugins/setup.rs`).
//!
//! 3. **`on_prey_animal_added`** — sibling observer for prey (140
//!    step 10).

use bevy_ecs::lifecycle::Add;
use bevy_ecs::prelude::*;

use crate::components::movement_budget::MovementBudget;
use crate::components::physical::{Needs, Position};
use crate::components::wildlife::WildAnimal;

/// Save-load lazy-insert pass (140 step 13 — the per-tick
/// `accumulate_movement_budget` accumulator loop this system replaces
/// is retired with the accumulator itself; only the compat inserts
/// remain).
///
/// Justified as per-tick under `docs/systems/ecs-rules.md`'s "default
/// event-driven, justify per-tick" rule by cost, not by shape: all
/// three queries filter on `Without<component>`, which Bevy resolves
/// at the archetype level — after the first post-load tick populates
/// the missing components, every query is empty and the pass is a
/// no-op. An event-driven load hook would save nothing measurable and
/// lose the test-scaffold coverage (schedules built without the save
/// pipeline still get components inserted on tick 1).
#[allow(clippy::type_complexity)]
pub fn insert_missing_movement_components(
    mut commands: Commands,
    wildlife_missing_budget: Query<(Entity, &WildAnimal), Without<MovementBudget>>,
    cats_missing_budget: Query<
        Entity,
        (
            With<Needs>,
            With<Position>,
            Without<MovementBudget>,
            Without<WildAnimal>,
        ),
    >,
    // 140 step 6 — anything with a Position but no DesiredVelocity
    // (pre-140 saves, test scaffolds).
    movers_missing_desire: Query<
        Entity,
        (
            With<Position>,
            With<Needs>,
            Without<crate::components::physical::DesiredVelocity>,
            Without<crate::components::building::Structure>,
        ),
    >,
    constants: Res<crate::resources::SimConstants>,
) {
    // Lazy-insert: pre-138 saves loaded with no MovementBudget. The
    // observers handle new spawns; this handles deserialized entities.
    for (entity, animal) in &wildlife_missing_budget {
        commands.entity(entity).insert(MovementBudget::for_species(
            animal.species,
            &constants.movement,
        ));
    }
    for entity in &cats_missing_budget {
        commands.entity(entity).insert(MovementBudget::cat());
    }
    // 140 step 6 — same lazy-insert shape for the fluid-movement pair.
    // Live spawns get them from the blueprint bundle; save-loaded cats
    // (pre-140 saves) and test scaffolds gain them here so the goap /
    // disposition queries (which REQUIRE DesiredVelocity) re-admit
    // them after one tick.
    for entity in &movers_missing_desire {
        commands.entity(entity).insert((
            crate::components::physical::Velocity::default(),
            crate::components::physical::DesiredVelocity::default(),
        ));
    }
}

/// Observer: insert species-default `MovementBudget` whenever a
/// `WildAnimal` is added to an entity. Registered in
/// `SimulationPlugin::build` via `app.add_observer`.
///
/// 140 step 9 — also the single author point for the fluid-movement
/// component pair on wildlife (`Velocity` + `DesiredVelocity`; the
/// species dispatchers REQUIRE `DesiredVelocity`, so a missing insert
/// would silently drop the animal from its GOAP resolver query), and
/// for the `Flying` marker on hawks (terrain-exempt integrator branch;
/// step 10 extends `Flying` to prey birds at their own spawn author).
pub fn on_wild_animal_added(
    add: On<Add, WildAnimal>,
    animals: Query<&WildAnimal>,
    mut commands: Commands,
    constants: Res<crate::resources::SimConstants>,
) {
    let entity = add.entity;
    if let Ok(animal) = animals.get(entity) {
        commands.entity(entity).insert((
            MovementBudget::for_species(animal.species, &constants.movement),
            crate::components::physical::Velocity::default(),
            crate::components::physical::DesiredVelocity::default(),
        ));
        if animal.species == crate::components::wildlife::WildSpecies::Hawk {
            commands
                .entity(entity)
                .insert(crate::components::physical::Flying);
        }
    }
}

/// Observer: fluid-movement author point for prey (140 step 10) —
/// sibling of [`on_wild_animal_added`]. Every `PreyAnimal` gains
/// `Velocity` + `DesiredVelocity` (the integrator and `prey_ai`'s
/// desire writes require them; a missing insert silently drops the
/// animal from `prey_ai`'s query) plus a `MovementBudget` speed cap:
///
/// - **Birds** (`FleeStrategy::Teleport` → `BurstFlight`): cap at
///   `bird_burst_speed` and carry [`Flying`] (terrain-exempt,
///   bounds-clamp-only integrator branch).
/// - **Ground prey**: cap at `prey_ground_max_speed × flee_speed` —
///   `flee_speed` was the pre-140 tiles-per-tick flee multiplier
///   (rabbit 2, mouse 1); folding it into the cap preserves each
///   species' escape-speed contrast vs the 1.0 cat.
pub fn on_prey_animal_added(
    add: On<Add, crate::components::prey::PreyAnimal>,
    configs: Query<&crate::components::prey::PreyConfig>,
    mut commands: Commands,
    constants: Res<crate::resources::SimConstants>,
) {
    let entity = add.entity;
    let Ok(config) = configs.get(entity) else {
        return;
    };
    let movement = &constants.movement;
    let is_bird = config.flee_strategy == crate::components::prey::FleeStrategy::Teleport;
    let per_tick = if is_bird {
        movement.bird_burst_speed
    } else {
        movement.prey_ground_max_speed * config.flee_speed.max(1) as f32
    };
    commands.entity(entity).insert((
        MovementBudget { per_tick },
        crate::components::physical::Velocity::default(),
        crate::components::physical::DesiredVelocity::default(),
    ));
    if is_bird {
        commands
            .entity(entity)
            .insert(crate::components::physical::Flying);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::wildlife::{BehaviorType, WildSpecies};

    #[test]
    fn lazy_insert_writes_species_default_for_wildlife() {
        let mut world = World::new();
        world.insert_resource(crate::resources::SimConstants::default());
        let entity = world
            .spawn((
                WildAnimal {
                    species: WildSpecies::Snake,
                    behavior: BehaviorType::Ambush,
                    threat_power: 0.08,
                    defense: 0.10,
                    ambush_cooldown: 0,
                },
                Position::new(0, 0),
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(insert_missing_movement_components);
        schedule.run(&mut world);
        let b = world
            .get::<MovementBudget>(entity)
            .expect("lazy-insert should write a budget");
        assert!((b.per_tick - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn lazy_insert_is_noop_for_already_authored_entities() {
        let mut world = World::new();
        world.insert_resource(crate::resources::SimConstants::default());
        let entity = world
            .spawn((MovementBudget { per_tick: 0.7 }, Position::new(0, 0)))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(insert_missing_movement_components);
        schedule.run(&mut world);
        let b = world.get::<MovementBudget>(entity).unwrap();
        assert!(
            (b.per_tick - 0.7).abs() < f32::EPSILON,
            "existing budget must not be overwritten or mutated"
        );
    }
}
