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
    stores: Query<&Position, (With<Structure>, Without<WildAnimal>, Without<FoxState>)>,
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

/// Author `HasCubs` per fox.
///
/// **Predicate** — the mother fox at a fox den whose
/// `cubs_present > 0`.
///
/// **Authoring (Ticket 050)** — hybrid event-driven + per-marker
/// reconciliation. Insertion fires off `CubsBorn` events emitted by
/// `wildlife.rs::breed_at_dens` at the moment of spawn so the marker
/// is set the same frame the litter exists. Removal walks only foxes
/// that *already* hold `HasCubs`: when the fox lost its den, or its
/// den's `cubs_present` decayed to 0 (cub maturation / death), the
/// marker drops. The full-fox scan retires — the reconciliation pass
/// is `O(mothers)`, not `O(foxes)`.
#[allow(clippy::type_complexity)]
pub fn update_cub_marker(
    mut commands: Commands,
    mut cubs_born_r: bevy_ecs::message::MessageReader<crate::messages::fox_lifecycle::CubsBorn>,
    holders: Query<(Entity, &FoxState, Has<markers::HasCubs>), (With<WildAnimal>, Without<Dead>)>,
    dens: Query<&crate::components::wildlife::FoxDen, Without<FoxState>>,
) {
    // Insertion: every `CubsBorn` event names the mother directly.
    for event in cubs_born_r.read() {
        if holders.get(event.mother).is_ok() {
            commands.entity(event.mother).insert(markers::HasCubs);
        }
    }

    // Removal: walk only foxes currently flagged. A flagged fox loses
    // the marker when its den is gone, or its den's cubs_present is 0
    // (cub matured / cub died).
    for (entity, fox_state, has_marker) in holders.iter() {
        if !has_marker {
            continue;
        }
        let want = fox_state
            .home_den
            .and_then(|e| dens.get(e).ok())
            .is_some_and(|d| d.cubs_present > 0);
        if !want {
            commands.entity(entity).remove::<markers::HasCubs>();
        }
    }
}

/// Author `CubsHungry` per fox.
///
/// **Predicate** — bit-for-bit mirror of
/// `fox_goap.rs::build_scoring_context::cubs_hungry`:
/// `has_cubs ∧ FoxNeeds.cub_satiation < 0.4`. The 0.4 threshold is
/// the inline literal at the field site; not migrated to a constant
/// in this commit.
#[allow(clippy::type_complexity)]
pub fn update_cub_hunger_markers(
    mut commands: Commands,
    foxes: Query<
        (
            Entity,
            &FoxState,
            &crate::components::fox_personality::FoxNeeds,
            Has<markers::CubsHungry>,
        ),
        (With<WildAnimal>, Without<Dead>),
    >,
    dens: Query<&FoxDen, Without<FoxState>>,
) {
    const CUB_HUNGER_THRESHOLD: f32 = 0.4;
    for (entity, fox_state, needs, has_marker) in foxes.iter() {
        let has_cubs = fox_state
            .home_den
            .and_then(|e| dens.get(e).ok())
            .map(|d| d.cubs_present > 0)
            .unwrap_or(false);
        let want = has_cubs && needs.cub_satiation < CUB_HUNGER_THRESHOLD;
        toggle(&mut commands, entity, want, has_marker, markers::CubsHungry);
    }
}

/// Author `IsDispersingJuvenile` per fox.
///
/// **Predicate** — bit-for-bit mirror of
/// `fox_goap.rs::build_scoring_context::is_dispersing_juvenile`:
/// `life_stage == Juvenile ∧ home_den.is_none()`.
#[allow(clippy::type_complexity)]
pub fn update_juvenile_dispersal_markers(
    mut commands: Commands,
    foxes: Query<
        (Entity, &FoxState, Has<markers::IsDispersingJuvenile>),
        (With<WildAnimal>, Without<Dead>),
    >,
) {
    use crate::components::wildlife::FoxLifeStage;
    for (entity, fox_state, has_marker) in foxes.iter() {
        let want = fox_state.life_stage == FoxLifeStage::Juvenile && fox_state.home_den.is_none();
        toggle(
            &mut commands,
            entity,
            want,
            has_marker,
            markers::IsDispersingJuvenile,
        );
    }
}

/// Author `HasDen` per fox — event-driven (Ticket 050).
///
/// **Predicate** — `fox_state.home_den.is_some()`.
///
/// **Authoring** — Pure event-driven. `DenClaimed` events from
/// `wildlife.rs` (initial pair spawn, cub birth, runtime adoption)
/// insert the marker; `DenLost` events (cub maturation, fox death,
/// future abandonment) remove it. The previous full-fox per-tick
/// scan retires. Dead-fox guard remains on insertion to be defensive
/// against an emit-after-death race.
#[allow(clippy::type_complexity)]
pub fn update_den_marker(
    mut commands: Commands,
    mut den_claimed_r: bevy_ecs::message::MessageReader<crate::messages::fox_lifecycle::DenClaimed>,
    mut den_lost_r: bevy_ecs::message::MessageReader<crate::messages::fox_lifecycle::DenLost>,
    holders: Query<Has<markers::HasDen>, (With<WildAnimal>, With<FoxState>, Without<Dead>)>,
) {
    for event in den_claimed_r.read() {
        // Defensive: only insert on live fox entities; dead foxes that
        // emitted `DenLost` in the same frame should not re-flag.
        if holders.get(event.fox).is_ok() {
            commands.entity(event.fox).insert(markers::HasDen);
        }
    }
    for event in den_lost_r.read() {
        // Removal is idempotent — `remove::<HasDen>` on an entity
        // that's already despawned or already lacks the marker is a
        // no-op.
        commands.entity(event.fox).remove::<markers::HasDen>();
    }
}

/// Author `WardNearbyFox` per fox.
///
/// **Predicate** — Ticket 050: any ward whose `repel_radius()`
/// reaches the fox's tile. Reads each ward's per-kind base radius
/// scaled by its current `strength`, so a decayed ward stops
/// asserting the marker even before it despawns. Inverted wards
/// (predator-attracting) still emit a detectable signal, so they
/// also flip the marker — semantically, the fox can sense the
/// magical presence either way; fox-side behavior responding to
/// the ward (flee vs. approach) is left to future DSE wiring
/// (no DSE consumes `WardNearbyFox` today).
///
/// **v1 is per-tick scan.** Event-driven authoring (`WardPlaced` /
/// `WardDespawned`) is a future refinement; per-tick scan keeps
/// the slice atomic.
#[allow(clippy::type_complexity)]
pub fn update_ward_detection_markers(
    mut commands: Commands,
    foxes: Query<
        (Entity, &Position, Has<markers::WardNearbyFox>),
        (With<WildAnimal>, With<FoxState>, Without<Dead>),
    >,
    wards: Query<
        (&crate::components::magic::Ward, &Position),
        (Without<WildAnimal>, Without<FoxState>),
    >,
) {
    // Snapshot ward positions + per-ward effective radii (Manhattan
    // tiles, rounded up so a ward with a 0.8-strength durable kind
    // still asserts on the integer boundary).
    let ward_snapshot: Vec<(Position, i32)> = wards
        .iter()
        .map(|(w, p)| (*p, w.repel_radius().ceil() as i32))
        .collect();

    for (entity, fox_pos, has_marker) in foxes.iter() {
        let want = ward_snapshot
            .iter()
            .any(|(wp, radius)| fox_pos.manhattan_distance(wp) <= *radius);
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
        world
            .entity_mut(den)
            .get_mut::<FoxDen>()
            .unwrap()
            .cubs_present = 0;
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::CatThreateningDen>());
    }

    fn setup_ward_detection() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_ward_detection_markers);
        (world, schedule)
    }

    fn spawn_ward(
        world: &mut World,
        x: i32,
        y: i32,
        kind: crate::components::magic::WardKind,
    ) -> Entity {
        let ward = match kind {
            crate::components::magic::WardKind::Thornward => {
                crate::components::magic::Ward::thornward()
            }
            crate::components::magic::WardKind::DurableWard => {
                crate::components::magic::Ward::durable()
            }
        };
        world.spawn((ward, Position::new(x, y))).id()
    }

    #[test]
    fn no_wards_no_marker() {
        let (mut world, mut schedule) = setup_ward_detection();
        let fox = spawn_fox(&mut world, 0, 0);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::WardNearbyFox>());
    }

    #[test]
    fn fox_inside_thornward_repel_radius_gets_marker() {
        let (mut world, mut schedule) = setup_ward_detection();
        let fox = spawn_fox(&mut world, 0, 0);
        // Thornward repel_radius == 6.0 at full strength.
        let _ward = spawn_ward(
            &mut world,
            5,
            0,
            crate::components::magic::WardKind::Thornward,
        );
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::WardNearbyFox>());
    }

    #[test]
    fn fox_outside_thornward_repel_radius_no_marker() {
        let (mut world, mut schedule) = setup_ward_detection();
        let fox = spawn_fox(&mut world, 0, 0);
        // Thornward repel_radius == 6.0; fox at distance 7 is outside.
        let _ward = spawn_ward(
            &mut world,
            7,
            0,
            crate::components::magic::WardKind::Thornward,
        );
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::WardNearbyFox>());
    }

    #[test]
    fn durable_ward_reaches_further_than_thornward() {
        let (mut world, mut schedule) = setup_ward_detection();
        let fox = spawn_fox(&mut world, 0, 0);
        // DurableWard repel_radius == 9.0; fox at distance 8 is inside.
        let _ward = spawn_ward(
            &mut world,
            8,
            0,
            crate::components::magic::WardKind::DurableWard,
        );
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::WardNearbyFox>());
    }

    #[test]
    fn decayed_ward_loses_marker_reach() {
        let (mut world, mut schedule) = setup_ward_detection();
        let fox = spawn_fox(&mut world, 0, 0);
        // Hand-spawn a thornward with strength 0.3 → repel_radius 1.8
        // (ceil to 2 tiles). Fox at distance 5 is well outside.
        let mut decayed = crate::components::magic::Ward::thornward();
        decayed.strength = 0.3;
        world.spawn((decayed, Position::new(5, 0)));
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
        // Even with a ward right next door, dead foxes are excluded.
        let _ward = spawn_ward(
            &mut world,
            0,
            1,
            crate::components::magic::WardKind::Thornward,
        );
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::WardNearbyFox>());
    }

    // -----------------------------------------------------------------------
    // Fox lifecycle markers — HasCubs / CubsHungry / IsDispersingJuvenile / HasDen
    // -----------------------------------------------------------------------

    use crate::components::fox_personality::FoxNeeds;
    use crate::components::wildlife::{FoxLifeStage, FoxSex};

    fn setup_cub_marker() -> (World, Schedule) {
        let mut world = World::new();
        // Ticket 050: event-driven authoring needs the Messages
        // resource pre-registered. The production plugin
        // (`SimulationPlugin::build`) calls `add_message::<CubsBorn>()`;
        // unit tests bootstrap manually.
        world
            .init_resource::<bevy_ecs::message::Messages<crate::messages::fox_lifecycle::CubsBorn>>(
            );
        let mut schedule = Schedule::default();
        schedule.add_systems(update_cub_marker);
        (world, schedule)
    }

    fn write_cubs_born(world: &mut World, mother: Entity, den: Entity) {
        world
            .resource_mut::<bevy_ecs::message::Messages<crate::messages::fox_lifecycle::CubsBorn>>()
            .write(crate::messages::fox_lifecycle::CubsBorn {
                mother,
                den,
                count: 1,
                position: Position::new(0, 0),
                tick: 0,
            });
    }

    #[test]
    fn cubs_born_event_inserts_has_cubs() {
        let (mut world, mut schedule) = setup_cub_marker();
        let den = spawn_den(&mut world, 5, 5, 2);
        let fox = spawn_fox_with_den(&mut world, 5, 5, den);
        write_cubs_born(&mut world, fox, den);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::HasCubs>());
    }

    #[test]
    fn no_cubs_born_event_no_marker() {
        let (mut world, mut schedule) = setup_cub_marker();
        let den = spawn_den(&mut world, 5, 5, 2);
        let fox = spawn_fox_with_den(&mut world, 5, 5, den);
        // No event; no scan: the marker stays absent.
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::HasCubs>());
    }

    #[test]
    fn cubs_present_decay_to_zero_removes_marker() {
        let (mut world, mut schedule) = setup_cub_marker();
        let den = spawn_den(&mut world, 5, 5, 2);
        let fox = spawn_fox_with_den(&mut world, 5, 5, den);
        write_cubs_born(&mut world, fox, den);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::HasCubs>());

        // Den's cubs decay to zero (cub matured / died). The
        // reconciliation pass over flagged foxes drops the marker.
        world
            .entity_mut(den)
            .get_mut::<FoxDen>()
            .unwrap()
            .cubs_present = 0;
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::HasCubs>());
    }

    #[test]
    fn losing_home_den_removes_has_cubs() {
        let (mut world, mut schedule) = setup_cub_marker();
        let den = spawn_den(&mut world, 5, 5, 2);
        let fox = spawn_fox_with_den(&mut world, 5, 5, den);
        write_cubs_born(&mut world, fox, den);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::HasCubs>());

        // Mother loses her den (abandoned / displaced); reconciliation
        // pass clears HasCubs.
        world
            .entity_mut(fox)
            .get_mut::<FoxState>()
            .unwrap()
            .home_den = None;
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::HasCubs>());
    }

    fn setup_cub_hunger() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_cub_hunger_markers);
        (world, schedule)
    }

    fn spawn_fox_with_needs(
        world: &mut World,
        fx: i32,
        fy: i32,
        den: Entity,
        cub_satiation: f32,
    ) -> Entity {
        let mut needs = FoxNeeds::default();
        needs.cub_satiation = cub_satiation;
        world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                FoxState::new_adult(FoxSex::Female, Some(den)),
                Position::new(fx, fy),
                needs,
            ))
            .id()
    }

    #[test]
    fn fox_no_cubs_no_cubs_hungry() {
        let (mut world, mut schedule) = setup_cub_hunger();
        let den = spawn_den(&mut world, 5, 5, 0);
        let fox = spawn_fox_with_needs(&mut world, 5, 5, den, 0.0);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::CubsHungry>());
    }

    #[test]
    fn cubs_well_fed_no_marker() {
        let (mut world, mut schedule) = setup_cub_hunger();
        let den = spawn_den(&mut world, 5, 5, 2);
        let fox = spawn_fox_with_needs(&mut world, 5, 5, den, 0.8); // > 0.4
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::CubsHungry>());
    }

    #[test]
    fn cubs_below_threshold_get_marker() {
        let (mut world, mut schedule) = setup_cub_hunger();
        let den = spawn_den(&mut world, 5, 5, 2);
        let fox = spawn_fox_with_needs(&mut world, 5, 5, den, 0.3); // < 0.4
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::CubsHungry>());
    }

    fn setup_juvenile_dispersal() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_juvenile_dispersal_markers);
        (world, schedule)
    }

    fn spawn_juvenile_no_den(world: &mut World) -> Entity {
        let mut state = FoxState::new_adult(FoxSex::Female, None);
        state.life_stage = FoxLifeStage::Juvenile;
        world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                state,
                Position::new(0, 0),
            ))
            .id()
    }

    fn spawn_juvenile_with_den(world: &mut World, den: Entity) -> Entity {
        let mut state = FoxState::new_adult(FoxSex::Female, Some(den));
        state.life_stage = FoxLifeStage::Juvenile;
        world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                state,
                Position::new(0, 0),
            ))
            .id()
    }

    #[test]
    fn juvenile_no_den_gets_dispersal_marker() {
        let (mut world, mut schedule) = setup_juvenile_dispersal();
        let fox = spawn_juvenile_no_den(&mut world);
        schedule.run(&mut world);
        assert!(world
            .entity(fox)
            .contains::<markers::IsDispersingJuvenile>());
    }

    #[test]
    fn juvenile_with_den_no_dispersal() {
        let (mut world, mut schedule) = setup_juvenile_dispersal();
        let den = spawn_den(&mut world, 5, 5, 0);
        let fox = spawn_juvenile_with_den(&mut world, den);
        schedule.run(&mut world);
        assert!(!world
            .entity(fox)
            .contains::<markers::IsDispersingJuvenile>());
    }

    #[test]
    fn adult_no_den_no_dispersal() {
        let (mut world, mut schedule) = setup_juvenile_dispersal();
        let fox = spawn_fox(&mut world, 0, 0);
        schedule.run(&mut world);
        // Adults that lose their den don't get the dispersal marker —
        // it's a juvenile-specific lifecycle stage.
        assert!(!world
            .entity(fox)
            .contains::<markers::IsDispersingJuvenile>());
    }

    fn setup_has_den() -> (World, Schedule) {
        let mut world = World::new();
        world.init_resource::<bevy_ecs::message::Messages<
            crate::messages::fox_lifecycle::DenClaimed,
        >>();
        world
            .init_resource::<bevy_ecs::message::Messages<crate::messages::fox_lifecycle::DenLost>>(
            );
        let mut schedule = Schedule::default();
        schedule.add_systems(update_den_marker);
        (world, schedule)
    }

    fn write_den_claimed(world: &mut World, fox: Entity, den: Entity) {
        world
            .resource_mut::<bevy_ecs::message::Messages<
                crate::messages::fox_lifecycle::DenClaimed,
            >>()
            .write(crate::messages::fox_lifecycle::DenClaimed {
                fox,
                den,
                position: Position::new(0, 0),
                tick: 0,
            });
    }

    fn write_den_lost(world: &mut World, fox: Entity, den: Entity) {
        world
            .resource_mut::<bevy_ecs::message::Messages<crate::messages::fox_lifecycle::DenLost>>()
            .write(crate::messages::fox_lifecycle::DenLost {
                fox,
                den,
                reason: crate::messages::fox_lifecycle::DenLostReason::Maturation,
                tick: 0,
            });
    }

    #[test]
    fn den_claimed_event_inserts_marker() {
        let (mut world, mut schedule) = setup_has_den();
        let den = spawn_den(&mut world, 5, 5, 0);
        let fox = spawn_fox_with_den(&mut world, 5, 5, den);
        write_den_claimed(&mut world, fox, den);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::HasDen>());
    }

    #[test]
    fn den_lost_event_removes_marker() {
        let (mut world, mut schedule) = setup_has_den();
        let den = spawn_den(&mut world, 5, 5, 0);
        let fox = spawn_fox_with_den(&mut world, 5, 5, den);
        write_den_claimed(&mut world, fox, den);
        schedule.run(&mut world);
        assert!(world.entity(fox).contains::<markers::HasDen>());

        write_den_lost(&mut world, fox, den);
        schedule.run(&mut world);
        assert!(!world.entity(fox).contains::<markers::HasDen>());
    }

    #[test]
    fn no_events_no_marker() {
        let (mut world, mut schedule) = setup_has_den();
        let _fox = spawn_fox(&mut world, 0, 0);
        // Without any DenClaimed event the marker stays absent.
        schedule.run(&mut world);
    }
}
