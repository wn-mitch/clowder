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
//! **Injury rule (2026-05-06, ticket 184):** Injury is *not* a
//! `CanHunt` gate. The same dissuades-not-disables principle applies
//! at the scoring layer (skill / health interoception dampen Hunt's
//! L2 appeal) so an injured cat can still elect Hunt when nothing
//! else is competitive — they still need to eat. The other three
//! capabilities (`CanForage`, `CanWard`, `CanCook`) retain the
//! `¬Injured` gate today; revisit if a similar action-share
//! cascade surfaces for them.
//!
//! **CanCook** is purely per-cat (`Adult ∧ ¬Injured`). Colony-scoped
//! checks (`HasFunctionalKitchen`, `HasRawFoodInStores`) stay on the
//! CookDse's `EligibilityFilter` so the `wants_cook_but_no_kitchen`
//! build-pressure signal in `scoring.rs` is preserved.

use bevy_ecs::prelude::*;

use crate::components::identity::Species;
use crate::components::markers::{
    Adult, CanCook, CanCraft, CanDry, CanForage, CanHunt, CanSmoke, CanWard, CanWardFromSupply,
    ColonyState, HasStoredThornbriar, HasWardHerbs, InCombat, Injured, JuvenileKitten, Kitten,
    Young,
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
            // Life-stage + injury inputs. 450 added `Has<JuvenileKitten>`
            // — Bevy's `QueryData` impl caps at 15 top-level fields, so
            // we bundle the kitten-stage axes into a nested tuple. Same
            // shape applies to the `cur_*` cluster below.
            (Has<Adult>, Has<Young>, Has<Kitten>, Has<JuvenileKitten>),
            Has<Injured>,
            Has<InCombat>,
            Has<HasWardHerbs>,
            Has<CanHunt>,
            Has<CanForage>,
            Has<CanWard>,
            Has<CanWardFromSupply>,
            Has<CanCook>,
            Has<CanDry>,
            Has<CanSmoke>,
            // 457 — Workshop-craft per-cat capability. Same `Adult ∧
            // ¬Injured` gate as CanCook / CanDry / CanSmoke.
            Has<CanCraft>,
        ),
        (With<Species>, Without<Dead>),
    >,
    // 084: colony-scope read for `CanWardFromSupply`. The colony
    // singleton is spawned once at world init; if it's somehow missing
    // (test paths that never spawn `ColonyState`), `has_stored_thornbriar`
    // stays `false` and `CanWardFromSupply` degrades to `CanWard`-with-
    // herbs-only behaviour — safe default.
    colony: Query<Has<HasStoredThornbriar>, With<ColonyState>>,
    map: Res<TileMap>,
    constants: Res<SimConstants>,
) {
    let d = &constants.disposition;
    let has_stored_thornbriar = colony.iter().any(|h| h);

    for (
        entity,
        pos,
        (is_adult, is_young, is_kitten, is_juvenile_kitten),
        is_injured,
        in_combat,
        has_ward_herbs,
        cur_hunt,
        cur_forage,
        cur_ward,
        cur_ward_from_supply,
        cur_cook,
        cur_dry,
        cur_smoke,
        cur_craft,
    ) in cats.iter()
    {
        // CanHunt: (Adult ∨ Young) ∧ ¬InCombat ∧ forest nearby.
        // Injury is intentionally NOT a gate (ticket 184 finding,
        // 2026-05-06): injured cats can still need to eat. Injury
        // dissuades, doesn't disable — a mangy one-eyed cat still
        // hunts rats. The skill / health interoception signals the
        // L2 scoring layer reads already dampen Hunt's appeal for
        // injured cats; gating eligibility on top of that
        // double-counted, and the absence of Hunt for injured cats
        // shifted action-share to Patrol (Blind commitment + long
        // plans amplified the gap into a +15pp share gain in the
        // post-181 soak).
        let want_hunt = (is_adult || is_young)
            && !in_combat
            && has_nearby_tile(pos, &map, d.hunt_terrain_search_radius, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            });
        toggle(&mut commands, entity, want_hunt, cur_hunt, CanHunt);

        // CanForage: (¬Kitten ∨ JuvenileKitten) ∧ ¬Injured ∧ forageable terrain.
        // 450 — Stage 3 (Juvenile) kittens widen the gate alongside
        // Young / Adult; Newborn / Eyes-open kittens stay excluded via
        // the broader Kitten marker.
        let want_forage = (!is_kitten || is_juvenile_kitten)
            && !is_injured
            && has_nearby_tile(pos, &map, d.forage_terrain_search_radius, |t| {
                t.foraging_yield() > 0.0
            });
        toggle(&mut commands, entity, want_forage, cur_forage, CanForage);

        // CanWard: Adult ∧ ¬Injured ∧ HasWardHerbs
        let want_ward = is_adult && !is_injured && has_ward_herbs;
        toggle(&mut commands, entity, want_ward, cur_ward, CanWard);

        // 084: CanWardFromSupply expands CanWard to cover cats who can
        // reach a stashed thornbriar — the GOAP planner branches into
        // either the carry-direct or retrieve-first chain depending on
        // which `CarryingIs` precondition holds at plan time.
        let want_ward_supply = is_adult && !is_injured && (has_ward_herbs || has_stored_thornbriar);
        toggle(
            &mut commands,
            entity,
            want_ward_supply,
            cur_ward_from_supply,
            CanWardFromSupply,
        );

        // CanCook: Adult ∧ ¬Injured (colony checks stay on CookDse)
        let want_cook = is_adult && !is_injured;
        toggle(&mut commands, entity, want_cook, cur_cook, CanCook);

        // 367: CanDry / CanSmoke share CanCook's per-cat gate. Station
        // availability + carried inventory stay on the per-DSE
        // eligibility filter (matches the cook precedent — colony-side
        // gates kept off the per-cat capability marker so absence of a
        // station can later flow into BuildPressure rather than
        // silently shutting the capability off).
        let want_dry = is_adult && !is_injured;
        toggle(&mut commands, entity, want_dry, cur_dry, CanDry);
        let want_smoke = is_adult && !is_injured;
        toggle(&mut commands, entity, want_smoke, cur_smoke, CanSmoke);

        // 457: CanCraft shares the same per-cat gate. Colony-side
        // Workshop availability stays on `CraftAtWorkshopDse`'s
        // eligibility filter via `HasFunctionalWorkshop`.
        let want_craft = is_adult && !is_injured;
        toggle(&mut commands, entity, want_craft, cur_craft, CanCraft);
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

/// Manhattan-nearest tile in `radius` matching `predicate`. Used by
/// the §L2.10.7 cat-side ScoringContext builder to populate the
/// `LandmarkAnchor::NearestForageableCluster` anchor (Forage spatial
/// axis). Single linear scan; `radius²` work but capped at the
/// `forage_terrain_search_radius` constant.
pub fn nearest_matching_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    mut predicate: impl FnMut(Terrain) -> bool,
) -> Option<Position> {
    let mut best: Option<(Position, i32)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = from.x + dx;
            let y = from.y + dy;
            if !map.in_bounds(x, y) {
                continue;
            }
            if !predicate(map.get(x, y).terrain) {
                continue;
            }
            let pos = Position::new(x, y);
            let d = from.manhattan_distance(&pos);
            if best.is_none_or(|(_, cur)| d < cur) {
                best = Some((pos, d));
            }
        }
    }
    best.map(|(p, _)| p)
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

    /// 2026-05-06 (ticket 184): injury is no longer a CanHunt gate.
    /// Injured cats can still elect Hunt; the skill / health scoring
    /// signals dampen the L2 score so Hunt becomes less attractive
    /// without becoming impossible. The "mangy one-eyed cat still
    /// hunts rats" intuition. Pre-184 this asserted the opposite
    /// (`!world.entity(cat).contains::<CanHunt>()`); the assertion
    /// flipped with the gate removal.
    #[test]
    fn injured_adult_keeps_can_hunt() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, Injured));

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanHunt>());
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
        assert!(!world.entity(cat).contains::<CanWardFromSupply>());
        assert!(!world.entity(cat).contains::<CanCook>());
        assert!(!world.entity(cat).contains::<CanDry>());
        assert!(!world.entity(cat).contains::<CanSmoke>());
        assert!(!world.entity(cat).contains::<CanCraft>());
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
    // CanWardFromSupply (ticket 084 Commit 2)
    // -----------------------------------------------------------------------

    #[test]
    fn adult_with_ward_herbs_gets_can_ward_from_supply() {
        // Cat carrying thornbriar — fires CanWardFromSupply regardless
        // of colony stash state.
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, HasWardHerbs));

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanWardFromSupply>());
    }

    #[test]
    fn adult_with_stash_gets_can_ward_from_supply_without_herbs() {
        // Cat NOT carrying thornbriar but colony stash has it —
        // CanWardFromSupply still fires so the retrieve-path plan can
        // form.
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);
        world.spawn((ColonyState, HasStoredThornbriar));

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanWardFromSupply>());
        assert!(!world.entity(cat).contains::<CanWard>());
    }

    #[test]
    fn no_herbs_no_stash_no_can_ward_from_supply() {
        // Neither carrying nor stashed thornbriar — combined marker
        // stays absent and HerbcraftSetWard is ineligible.
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);
        world.spawn(ColonyState); // colony exists but no HasStoredThornbriar

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanWardFromSupply>());
    }

    #[test]
    fn injured_no_can_ward_from_supply_even_with_stash() {
        // Injury gates apply uniformly across the ward-eligibility
        // surface.
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, HasWardHerbs, Injured));
        world.spawn((ColonyState, HasStoredThornbriar));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanWardFromSupply>());
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
    // CanDry / CanSmoke (ticket 367)
    // -----------------------------------------------------------------------

    #[test]
    fn adult_gets_can_dry_and_can_smoke() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Adult);

        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanDry>());
        assert!(world.entity(cat).contains::<CanSmoke>());
    }

    #[test]
    fn young_no_preservation_capabilities() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert(Young);

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanDry>());
        assert!(!world.entity(cat).contains::<CanSmoke>());
    }

    #[test]
    fn injured_no_preservation_capabilities() {
        let (mut world, mut schedule) = setup();
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, Injured));

        schedule.run(&mut world);

        assert!(!world.entity(cat).contains::<CanDry>());
        assert!(!world.entity(cat).contains::<CanSmoke>());
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
        // Injured: CanHunt stays asserted (injury dissuades, doesn't
        // disable per ticket 184); CanWard still gates on injury.
        assert!(world.entity(cat).contains::<CanHunt>());
        assert!(!world.entity(cat).contains::<CanWard>());

        // Heal
        world.entity_mut(cat).remove::<Injured>();
        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanHunt>());
        assert!(world.entity(cat).contains::<CanWard>());
    }

    #[test]
    fn injury_transition_keeps_can_hunt_removes_can_ward() {
        let (mut world, mut schedule) = setup();
        set_terrain(&mut world, 11, 10, Terrain::DenseForest);
        let cat = spawn_cat(&mut world, 10, 10);
        world.entity_mut(cat).insert((Adult, HasWardHerbs));

        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<CanHunt>());
        assert!(world.entity(cat).contains::<CanWard>());

        // Get injured — Hunt stays available, Ward drops.
        world.entity_mut(cat).insert(Injured);
        schedule.run(&mut world);

        assert!(world.entity(cat).contains::<CanHunt>());
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
