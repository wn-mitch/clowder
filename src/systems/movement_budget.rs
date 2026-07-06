//! `MovementBudget` per-tick accumulator + wildlife spawn observer
//! (ticket 138, Phase 1 of the 135 continuous-position-migration epic).
//!
//! See `crate::components::movement_budget` for the component-level
//! contract. This module:
//!
//! 1. **`accumulate_movement_budget`** — per-tick system that calls
//!    `MovementBudget::accumulate` on every entity carrying the
//!    component. Justified as per-tick under
//!    `docs/systems/ecs-rules.md`'s "default event-driven, justify
//!    per-tick" rule: the budget is a continuous accumulator (same shape
//!    as `decay_needs`), not an event-triggered transition. Also
//!    serves as the lazy-insert path for save-loaded entities that
//!    pre-date this component (mirrors `update_prev_safety_deficit`'s
//!    pattern from ticket 108).
//!
//! 2. **`on_wild_animal_added`** — `OnAdd<WildAnimal>` observer that
//!    inserts a species-default `MovementBudget` whenever a `WildAnimal`
//!    is first added to an entity. Keeps the ~20 `WildAnimal::new`
//!    call-sites untouched — one canonical author for newly-spawned
//!    wildlife (substrate-stubs discipline: one writer per component).
//!    Cats are authored separately in `cat_bundle`
//!    (`src/plugins/setup.rs`) since they don't carry `WildAnimal`.

use bevy_ecs::lifecycle::Add;
use bevy_ecs::prelude::*;

use crate::components::movement_budget::MovementBudget;
use crate::components::physical::{Needs, Position};
use crate::components::wildlife::WildAnimal;

/// Per-tick accumulator pass. Runs early in Chain 1 (before
/// `wildlife_ai` and the species GOAP resolvers) so consumers spend a
/// freshly-ticked budget within the same tick.
///
/// Lazy-insert paths (one for wildlife, one for cats) catch save-loaded
/// entities from pre-138 saves. Live-spawned entities get the component
/// at spawn time via `on_wild_animal_added` or `cat_bundle`; the
/// lazy-insert path is only exercised on save-load.
#[allow(clippy::type_complexity)]
pub fn accumulate_movement_budget(
    mut commands: Commands,
    mut budgeted: Query<&mut MovementBudget>,
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
    // (pre-140 saves, test scaffolds). Wildlife included: species
    // migrations (plan steps 9-11) will read these on wildlife too.
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
    for mut budget in &mut budgeted {
        budget.accumulate();
    }
    // Lazy-insert: pre-138 saves loaded with no MovementBudget. The
    // observer handles new spawns; this handles deserialized entities.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::wildlife::{BehaviorType, WildSpecies};

    #[test]
    fn accumulator_ticks_existing_budgets() {
        let mut world = World::new();
        world.insert_resource(crate::resources::SimConstants::default());
        let entity = world
            .spawn(MovementBudget {
                accumulator: 0.0,
                per_tick: 0.5,
            })
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(accumulate_movement_budget);
        schedule.run(&mut world);
        let b = world.get::<MovementBudget>(entity).unwrap();
        assert!((b.accumulator - 0.5).abs() < f32::EPSILON);
    }

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
        schedule.add_systems(accumulate_movement_budget);
        schedule.run(&mut world);
        let b = world
            .get::<MovementBudget>(entity)
            .expect("lazy-insert should write a budget");
        assert!((b.per_tick - 0.5).abs() < f32::EPSILON);
    }
}
