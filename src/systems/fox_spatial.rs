//! §4 fox spatial-marker authoring (Ticket 014 Commit 5).
//!
//! Three per-tick author systems write fox-side §4 spatial ZSTs that
//! mirror today's `FoxScoringContext` field computations in
//! `fox_goap.rs::build_scoring_context`. Once authored, the
//! `MarkerSnapshot` populated inside `fox_evaluate_and_plan` reads the
//! markers, and per-fox FoxScoringContext fields read via
//! `markers.has(KEY, fox_entity)` instead of recomputing.
//!
//! **Markers authored here:**
//! - `StoreVisible` — fox sees a colony store within 12 tiles.
//! - `StoreGuarded` — at least one cat is within 5 tiles of any
//!   colony store. Per-fox marker but the predicate is colony-scoped
//!   (every fox sees the same value); kept per-fox for symmetry with
//!   the FoxScoringContext field.
//! - `CatThreateningDen` — fox has cubs at den AND a cat is within
//!   5 tiles of that den.
//! - `WardNearbyFox` — placeholder; predicate today is hardcoded
//!   `false` in the ScoringContext field. The author wires the
//!   marker to the same `false` value; future work that flips it to
//!   a truthful predicate is tracked in ticket 014's "WardNearbyFox"
//!   stub-promotion follow-on.

use bevy_ecs::prelude::*;

use crate::components::building::Structure;
use crate::components::markers;
use crate::components::physical::{Dead, Health, Position};
use crate::components::wildlife::{FoxDen, FoxState, WildAnimal};

fn toggle<M: Component + Copy>(
    commands: &mut Commands,
    entity: Entity,
    want: bool,
    has: bool,
    marker: M,
) {
    match (want, has) {
        (true, false) => {
            commands.entity(entity).insert(marker);
        }
        (false, true) => {
            commands.entity(entity).remove::<M>();
        }
        _ => {}
    }
}

/// Author `StoreVisible` and `StoreGuarded` per fox.
///
/// **Predicates** — bit-for-bit mirror of
/// `fox_goap.rs::build_scoring_context` lines for `store_visible` /
/// `store_guarded`:
/// - `StoreVisible` iff any colony store within 12 tiles Manhattan.
/// - `StoreGuarded` iff any colony store has any cat within 5 tiles.
///
/// **Ordering** — Chain 2a, after the per-cat marker authors.
#[allow(clippy::type_complexity)]
pub fn update_store_awareness_markers(
    mut commands: Commands,
    foxes: Query<
        (
            Entity,
            &Position,
            Has<markers::StoreVisible>,
            Has<markers::StoreGuarded>,
        ),
        (With<WildAnimal>, With<FoxState>, Without<Dead>),
    >,
    stores: Query<
        &Position,
        (With<Structure>, Without<WildAnimal>, Without<FoxState>),
    >,
    cats: Query<
        &Position,
        (
            Without<WildAnimal>,
            Without<FoxState>,
            With<Health>,
            Without<Dead>,
        ),
    >,
) {
    let store_positions: Vec<Position> = stores.iter().copied().collect();
    let cat_positions: Vec<Position> = cats.iter().copied().collect();

    for (entity, fox_pos, cur_visible, cur_guarded) in foxes.iter() {
        let want_visible = store_positions
            .iter()
            .any(|p| p.manhattan_distance(fox_pos) <= 12);
        let want_guarded = store_positions.iter().any(|sp| {
            cat_positions
                .iter()
                .any(|cp| cp.manhattan_distance(sp) <= 5)
        });
        toggle(
            &mut commands,
            entity,
            want_visible,
            cur_visible,
            markers::StoreVisible,
        );
        toggle(
            &mut commands,
            entity,
            want_guarded,
            cur_guarded,
            markers::StoreGuarded,
        );
    }
}

/// Author `CatThreateningDen` per fox.
///
/// **Predicate** — bit-for-bit mirror of
/// `fox_goap.rs::build_scoring_context::cat_threatening_den`:
/// `cubs_present > 0 ∧ ∃ cat : cat.manhattan_distance(den) ≤ 5`.
/// A fox without a `home_den` or with no cubs at it never gets the
/// marker.
#[allow(clippy::type_complexity)]
pub fn update_den_threat_markers(
    mut commands: Commands,
    foxes: Query<
        (Entity, &FoxState, Has<markers::CatThreateningDen>),
        (With<WildAnimal>, Without<Dead>),
    >,
    dens: Query<(Entity, &FoxDen, &Position), Without<FoxState>>,
    cats: Query<
        &Position,
        (
            Without<WildAnimal>,
            Without<FoxState>,
            With<Health>,
            Without<Dead>,
        ),
    >,
) {
    let cat_positions: Vec<Position> = cats.iter().copied().collect();

    for (entity, fox_state, has_marker) in foxes.iter() {
        let den_info = fox_state
            .home_den
            .and_then(|e| dens.get(e).ok())
            .map(|(_, d, p)| (*p, d.cubs_present));
        let want = match den_info {
            Some((den_pos, cubs_present)) if cubs_present > 0 => cat_positions
                .iter()
                .any(|cp| cp.manhattan_distance(&den_pos) <= 5),
            _ => false,
        };
        toggle(
            &mut commands,
            entity,
            want,
            has_marker,
            markers::CatThreateningDen,
        );
    }
}

/// Author `WardNearbyFox` per fox.
///
/// **Predicate** — currently hardcoded `false` to mirror today's
/// `FoxScoringContext.ward_nearby` stub at
/// `fox_goap.rs::build_scoring_context`. The marker is wired up so
/// future predicate-refinement work (truthful "ward within fox
/// detection radius" check) flips the value at this single site.
/// Today's call sites read `false` either way; the marker is
/// behavior-neutral.
#[allow(clippy::type_complexity)]
pub fn update_ward_detection_markers(
    mut commands: Commands,
    foxes: Query<
        (Entity, Has<markers::WardNearbyFox>),
        (With<WildAnimal>, With<FoxState>, Without<Dead>),
    >,
) {
    for (entity, has_marker) in foxes.iter() {
        // Stubbed predicate to match the existing
        // `FoxScoringContext.ward_nearby = false` baseline. When a
        // truthful predicate lands, swap this to a Ward-position scan.
        let want = false;
        toggle(
            &mut commands,
            entity,
            want,
            has_marker,
            markers::WardNearbyFox,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::DeathCause;
    use crate::components::wildlife::WildSpecies;
    use bevy_ecs::schedule::Schedule;

    fn setup_store_awareness() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_store_awareness_markers);
        (world, schedule)
    }

    fn spawn_fox(world: &mut World, x: i32, y: i32) -> Entity {
        world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                FoxState::new_adult(crate::components::wildlife::FoxSex::Male, None),
                Position::new(x, y),
            ))
            .id()
    }

    fn spawn_store(world: &mut World, x: i32, y: i32) -> Entity {
        world
            .spawn((
                Structure::new(crate::components::building::StructureType::Stores),
                Position::new(x, y),
            ))
            .id()
    }

    fn spawn_cat(world: &mut World, x: i32, y: i32) -> Entity {
        world.spawn((Position::new(x, y), Health::default())).id()
    }

    #[test]
    fn solo_fox_no_store_markers() {
        let (mut world, mut schedule) = setup_store_awareness();
        let fox = spawn_fox(&mut world, 0, 0);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::StoreVisible>());
        assert!(!world.entity(fox).contains::<markers::StoreGuarded>());
    }

    #[test]
    fn store_in_range_flags_visible() {
        let (mut world, mut schedule) = setup_store_awareness();
        let fox = spawn_fox(&mut world, 0, 0);
        let _store = spawn_store(&mut world, 10, 0);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::StoreVisible>());
        assert!(!world.entity(fox).contains::<markers::StoreGuarded>());
    }

    #[test]
    fn store_far_no_visible() {
        let (mut world, mut schedule) = setup_store_awareness();
        let fox = spawn_fox(&mut world, 0, 0);
        let _store = spawn_store(&mut world, 50, 0);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::StoreVisible>());
    }

    #[test]
    fn cat_near_store_flags_guarded() {
        let (mut world, mut schedule) = setup_store_awareness();
        let fox = spawn_fox(&mut world, 0, 0);
        let _store = spawn_store(&mut world, 10, 0);
        let _cat = spawn_cat(&mut world, 12, 0);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::StoreGuarded>());
    }

    #[test]
    fn cat_far_from_store_not_guarded() {
        let (mut world, mut schedule) = setup_store_awareness();
        let fox = spawn_fox(&mut world, 0, 0);
        let _store = spawn_store(&mut world, 10, 0);
        let _cat = spawn_cat(&mut world, 50, 50);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::StoreVisible>());
        assert!(!world.entity(fox).contains::<markers::StoreGuarded>());
    }

    #[test]
    fn dead_fox_excluded() {
        let (mut world, mut schedule) = setup_store_awareness();
        let fox = world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                FoxState::new_adult(crate::components::wildlife::FoxSex::Male, None),
                Position::new(0, 0),
                Dead {
                    tick: 0,
                    cause: DeathCause::Starvation,
                },
            ))
            .id();
        let _store = spawn_store(&mut world, 5, 0);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::StoreVisible>());
    }

    fn setup_den_threat() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_den_threat_markers);
        (world, schedule)
    }

    fn spawn_den(world: &mut World, x: i32, y: i32, cubs_present: u32) -> Entity {
        let mut den = FoxDen::new(20, 0);
        den.cubs_present = cubs_present;
        world.spawn((den, Position::new(x, y))).id()
    }

    fn spawn_fox_with_den(world: &mut World, fx: i32, fy: i32, den: Entity) -> Entity {
        world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                FoxState::new_adult(crate::components::wildlife::FoxSex::Male, Some(den)),
                Position::new(fx, fy),
            ))
            .id()
    }

    #[test]
    fn fox_no_den_no_threat_marker() {
        let (mut world, mut schedule) = setup_den_threat();
        let fox = spawn_fox(&mut world, 0, 0);
        let _cat = spawn_cat(&mut world, 1, 0);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::CatThreateningDen>());
    }

    #[test]
    fn fox_with_den_no_cubs_no_threat() {
        let (mut world, mut schedule) = setup_den_threat();
        let den = spawn_den(&mut world, 10, 10, 0);
        let fox = spawn_fox_with_den(&mut world, 10, 10, den);
        let _cat = spawn_cat(&mut world, 11, 10);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::CatThreateningDen>());
    }

    #[test]
    fn cat_near_den_with_cubs_triggers_threat() {
        let (mut world, mut schedule) = setup_den_threat();
        let den = spawn_den(&mut world, 10, 10, 2);
        let fox = spawn_fox_with_den(&mut world, 10, 10, den);
        let _cat = spawn_cat(&mut world, 11, 10);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::CatThreateningDen>());
    }

    #[test]
    fn cat_far_from_den_no_threat() {
        let (mut world, mut schedule) = setup_den_threat();
        let den = spawn_den(&mut world, 10, 10, 2);
        let fox = spawn_fox_with_den(&mut world, 10, 10, den);
        let _cat = spawn_cat(&mut world, 100, 100);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::CatThreateningDen>());
    }

    #[test]
    fn den_threat_clears_when_cubs_lost() {
        let (mut world, mut schedule) = setup_den_threat();
        let den = spawn_den(&mut world, 10, 10, 2);
        let fox = spawn_fox_with_den(&mut world, 10, 10, den);
        let _cat = spawn_cat(&mut world, 11, 10);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::CatThreateningDen>());
        // Cubs gone — marker drops.
        world.entity_mut(den).get_mut::<FoxDen>().unwrap().cubs_present = 0;
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::CatThreateningDen>());
    }

    fn setup_ward_detection() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_ward_detection_markers);
        (world, schedule)
    }

    #[test]
    fn ward_nearby_fox_starts_false() {
        // The author currently mirrors the pre-existing `false` stub.
        // This test pins that behavior so any future change of the
        // predicate is intentional.
        let (mut world, mut schedule) = setup_ward_detection();
        let fox = spawn_fox(&mut world, 0, 0);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::WardNearbyFox>());
    }

    #[test]
    fn dead_fox_no_ward_authoring() {
        let (mut world, mut schedule) = setup_ward_detection();
        let fox = world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                FoxState::new_adult(crate::components::wildlife::FoxSex::Male, None),
                Position::new(0, 0),
                Dead {
                    tick: 0,
                    cause: DeathCause::Starvation,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::WardNearbyFox>());
    }
}
