//! §4 Capability marker authoring — batch 2.
//!
//! **Four markers, one system.** All four share the same parent
//! components (`Species`, life-stage ZSTs, `Injured`, `Position`) so
//! a single query avoids redundant iteration. Each marker encodes a
//! conjunction of cat-intrinsic capability (life stage + injury +
//! inventory) and, for Hunt/Forage, spatial terrain availability.
//!
//! **Life-stage rules (design decision 2026-04-24):**
//! - *Young* cats can hunt (badly — skill gates outcome quality, not
//!   the capability marker) and forage.
//! - *Elders* forage but don't hunt (reduced physical capacity).
//! - *Kittens* are excluded from all four capabilities (fed by parents).
//!
//! **CanCook** is purely per-cat (`Adult ∧ ¬Injured`). Colony-scoped
//! checks (`HasFunctionalKitchen`, `HasRawFoodInStores`) stay on the
//! CookDse's `EligibilityFilter` so the `wants_cook_but_no_kitchen`
//! build-pressure signal in `scoring.rs` is preserved.

use bevy_ecs::prelude::*;

use crate::components::identity::Species;
use crate::components::markers::{
    Adult, CanCook, CanForage, CanHunt, CanWard, HasWardHerbs, InCombat, Injured, Kitten, Young,
};
use crate::components::physical::{Dead, Position};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::sim_constants::SimConstants;

/// Per-tick system that inserts/removes the four `Can*` capability
/// markers on every living cat. Must run **after** life-stage markers,
/// `update_injury_marker`, and `update_inventory_markers` (reads their
/// outputs), and **before** the GOAP/scoring pipeline (produces
/// `MarkerSnapshot` inputs).
#[allow(clippy::type_complexity)]
pub fn update_capability_markers(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            &Position,
            Has<Adult>,
            Has<Young>,
            Has<Kitten>,
            Has<Injured>,
            Has<InCombat>,
            Has<HasWardHerbs>,
            Has<CanHunt>,
            Has<CanForage>,
            Has<CanWard>,
            Has<CanCook>,
        ),
        (With<Species>, Without<Dead>),
    >,
    map: Res<TileMap>,
    constants: Res<SimConstants>,
) {
    let d = &constants.disposition;

    for (
        entity,
        pos,
        is_adult,
        is_young,
        is_kitten,
        is_injured,
        in_combat,
        has_ward_herbs,
        cur_hunt,
        cur_forage,
        cur_ward,
        cur_cook,
    ) in cats.iter()
    {
        // CanHunt: (Adult ∨ Young) ∧ ¬Injured ∧ ¬InCombat ∧ forest nearby
        let want_hunt = (is_adult || is_young)
            && !is_injured
            && !in_combat
            && has_nearby_tile(pos, &map, d.hunt_terrain_search_radius, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            });
        toggle(&mut commands, entity, want_hunt, cur_hunt, CanHunt);

        // CanForage: ¬Kitten ∧ ¬Injured ∧ forageable terrain nearby
        let want_forage = !is_kitten
            && !is_injured
            && has_nearby_tile(pos, &map, d.forage_terrain_search_radius, |t| {
                t.foraging_yield() > 0.0
            });
        toggle(&mut commands, entity, want_forage, cur_forage, CanForage);

        // CanWard: Adult ∧ ¬Injured ∧ HasWardHerbs
        let want_ward = is_adult && !is_injured && has_ward_herbs;
        toggle(&mut commands, entity, want_ward, cur_ward, CanWard);

        // CanCook: Adult ∧ ¬Injured (colony checks stay on CookDse)
        let want_cook = is_adult && !is_injured;
        toggle(&mut commands, entity, want_cook, cur_cook, CanCook);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Insert/remove a ZST marker only when state actually changes,
/// avoiding unnecessary archetype moves.
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

/// Early-exit terrain scan — returns `true` as soon as any tile within
/// `radius` matches `predicate`. Cheaper than `find_nearest_tile` when
/// we only need existence, not location.
fn has_nearby_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
) -> bool {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx == 0 && dy == 0 {
                continue;
            }
            let x = from.x + dx;
            let y = from.y + dy;
            if map.in_bounds(x, y) && predicate(map.get(x, y).terrain) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::identity::Species;
    use crate::components::markers;
    use crate::resources::map::{Terrain, TileMap};
    use crate::resources::sim_constants::SimConstants;
    use bevy_ecs::schedule::Schedule;

    /// Build a world with a TileMap and SimConstants. The map is 20×20
    /// grassland by default; callers stamp terrain via `set_terrain`.
    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        let map = TileMap::new(20, 20, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimConstants::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(update_capability_markers);
        (world, schedule)
    }

    fn set_terrain(world: &mut World, x: i32, y: i32, terrain: Terrain) {
        let mut map = world.resource_mut::<TileMap>();
        map.get_mut(x, y).terrain = terrain;
    }

    /// Spawn a living cat at the given position with the given marker
    /// components. Returns the entity.
    fn spawn_cat(world: &mut World, x: i32, y: i32) -> Entity {
        world.spawn((Species, Position::new(x, y))).id()
    }

    // -----------------------------------------------------------------------
    // CanHunt
    // -----------------------------------------------------------------------

    #[test]
    fn adult_near_forest_gets_can_hunt() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanHunt>());
    }

    #[test]
    fn young_near_forest_gets_can_hunt() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::LightForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Young);

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanHunt>());
    }

    #[test]
    fn kitten_no_can_hunt() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Kitten);

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanHunt>());
    }

    #[test]
    fn elder_no_can_hunt() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(markers::Elder);

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanHunt>());
    }

    #[test]
    fn injured_adult_no_can_hunt() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, Injured));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanHunt>());
    }

    #[test]
    fn in_combat_no_can_hunt() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, InCombat));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanHunt>());
    }

    #[test]
    fn adult_no_forest_no_can_hunt() {
        let (mut world, mut schedule) = setup();
        // All grass, no forest anywhere
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanHunt>());
    }

    #[test]
    fn dead_cat_no_markers() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = world
            .spawn((
                Species,
                Position::new(10, 10),
                Adult,
                Dead {
                    tick: 0,
                    cause: crate::components::physical::DeathCause::Starvation,
                },
            ))
            .id();

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanHunt>());
        assert!(!world.entity(cat).contains::<CanForage>());
        assert!(!world.entity(cat).contains::<CanWard>());
        assert!(!world.entity(cat).contains::<CanCook>());
    }

    // -----------------------------------------------------------------------
    // CanForage
    // -----------------------------------------------------------------------

    #[test]
    fn adult_forageable_gets_can_forage() {
        let (mut world, mut schedule) = setup();
        // DenseForest has foraging_yield 0.5
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanForage>());
    }

    #[test]
    fn young_forageable_gets_can_forage() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Young);

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanForage>());
    }

    #[test]
    fn elder_forageable_gets_can_forage() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(markers::Elder);

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanForage>());
    }

    #[test]
    fn kitten_no_can_forage() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Kitten);

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanForage>());
    }

    #[test]
    fn injured_no_can_forage() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, Injured));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanForage>());
    }

    // -----------------------------------------------------------------------
    // CanWard
    // -----------------------------------------------------------------------

    #[test]
    fn adult_ward_herbs_gets_can_ward() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, HasWardHerbs));

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanWard>());
    }

    #[test]
    fn no_ward_herbs_no_can_ward() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanWard>());
    }

    #[test]
    fn injured_no_can_ward() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, HasWardHerbs, Injured));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanWard>());
    }

    #[test]
    fn young_no_can_ward() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Young, HasWardHerbs));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanWard>());
    }

    // -----------------------------------------------------------------------
    // CanCook
    // -----------------------------------------------------------------------

    #[test]
    fn adult_gets_can_cook() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanCook>());
    }

    #[test]
    fn young_no_can_cook() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Young);

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanCook>());
    }

    #[test]
    fn injured_no_can_cook() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, Injured));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanCook>());
    }

    // -----------------------------------------------------------------------
    // Transition tests
    // -----------------------------------------------------------------------

    #[test]
    fn heal_transition_adds_markers() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, Injured, HasWardHerbs));

        schedule.run(&mut world);
        // Injured: no capabilities
        assert!(!world.entity(cat).contains::<CanHunt>());
        assert!(!world.entity(cat).contains::<CanWard>());

        // Heal
        world.entity_mut(cat).remove::<Injured>();
        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanHunt>());
        assert!(world.entity(cat).contains::<CanWard>());
    }

    #[test]
    fn injury_transition_removes_markers() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, HasWardHerbs));

        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<CanHunt>());
        assert!(world.entity(cat).contains::<CanWard>());

        // Get injured
        world.entity_mut(cat).insert(Injured);
        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanHunt>());
        assert!(!world.entity(cat).contains::<CanWard>());
    }

    #[test]
    fn capability_markers_idempotent() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);

        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<CanHunt>());

        // Run again with same state — no panic, same result
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<CanHunt>());
    }
}
