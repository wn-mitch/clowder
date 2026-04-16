use bevy_ecs::prelude::*;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::components::physical::{Health, Position};
use crate::components::prey::{PreyDen, PreyKind};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::rng::SimRng;
use crate::species::{PreyProfile, SpeciesRegistry};

// ---------------------------------------------------------------------------
// Poisson disk den placement
// ---------------------------------------------------------------------------

/// Place dens for a species using Poisson disk sampling on valid habitat tiles.
/// Shuffles candidates for randomness, enforces min spacing for even coverage.
fn seed_dens(
    habitat_tiles: &[(i32, i32)],
    min_spacing: i32,
    max_dens: usize,
    rng: &mut impl Rng,
) -> Vec<Position> {
    let mut dens: Vec<Position> = Vec::new();
    let mut candidates: Vec<(i32, i32)> = habitat_tiles.to_vec();
    candidates.shuffle(rng);

    for &(x, y) in &candidates {
        if dens.len() >= max_dens {
            break;
        }
        let pos = Position::new(x, y);
        let far_enough = dens
            .iter()
            .all(|d| pos.manhattan_distance(d) >= min_spacing);
        if far_enough {
            dens.push(pos);
        }
    }
    dens
}

// ---------------------------------------------------------------------------
// Ecosystem seeding (replaces spawn_initial_prey)
// ---------------------------------------------------------------------------

/// Seed the map with a mature prey ecosystem: dens distributed across all
/// suitable habitat via Poisson disk sampling, prey spread between them.
///
/// This models what the prey network would look like after years of
/// natural expansion — before the cat colony arrived.
pub fn seed_prey_ecosystem(world: &mut World) {
    // Snapshot terrain (release map borrow before spawning).
    let (map_width, map_height, terrain_snapshot): (i32, i32, Vec<Terrain>) = {
        let map = world.resource::<TileMap>();
        let snapshot = (0..map.height)
            .flat_map(|y| (0..map.width).map(move |x| (x, y)))
            .map(|(x, y)| map.get(x, y).terrain)
            .collect();
        (map.width, map.height, snapshot)
    };

    let terrain_at = |x: i32, y: i32| -> Terrain { terrain_snapshot[(y * map_width + x) as usize] };

    // We'll collect habitat tiles per species below (terrain isn't Hash).

    // Collect per-species data from registry (release borrow before spawning).
    struct SpeciesSeed {
        kind: PreyKind,
        den_habitat: &'static [Terrain],
        prey_habitat: &'static [Terrain],
        prey_count: usize,
        den_spacing: i32,
        den_density: usize,
        den_template: PreyDen,
    }

    let species_seeds: Vec<SpeciesSeed> = {
        let registry = world.resource::<SpeciesRegistry>();
        registry
            .profiles
            .iter()
            .map(|p| {
                let profile = p.as_ref();
                SpeciesSeed {
                    kind: profile.kind(),
                    den_habitat: profile.den_habitat(),
                    prey_habitat: profile.habitat(),
                    prey_count: profile.population_cap() / 2,
                    den_spacing: profile.den_spacing(),
                    den_density: profile.den_density(),
                    den_template: PreyDen::from_profile(profile),
                }
            })
            .collect()
    };

    // Place dens and collect spawn data.
    let mut den_spawns: Vec<(PreyDen, Position)> = Vec::new();
    let mut prey_spawns: Vec<(PreyKind, Position, usize)> = Vec::new(); // (kind, pos, den_index)

    {
        let rng = &mut world.resource_mut::<SimRng>().rng;

        for seed in &species_seeds {
            // Collect all habitat tiles for this species' dens.
            let habitat: Vec<(i32, i32)> = (0..map_height)
                .flat_map(|y| (0..map_width).map(move |x| (x, y)))
                .filter(|&(x, y)| seed.den_habitat.contains(&terrain_at(x, y)))
                .collect();

            if habitat.is_empty() {
                continue;
            }

            // Target den count from habitat area / density.
            let max_dens = (habitat.len() / seed.den_density).max(2);

            // Poisson disk placement.
            let den_positions = seed_dens(&habitat, seed.den_spacing, max_dens, rng);

            let den_start_idx = den_spawns.len();

            eprintln!(
                "  {:?}: {} habitat tiles → {} dens (spacing {}, density {})",
                seed.kind,
                habitat.len(),
                den_positions.len(),
                seed.den_spacing,
                seed.den_density,
            );

            for pos in &den_positions {
                den_spawns.push((seed.den_template.clone(), *pos));
            }

            // Distribute prey evenly across all dens.
            if den_positions.is_empty() {
                continue;
            }

            let prey_per_den = seed.prey_count / den_positions.len();
            let remainder = seed.prey_count % den_positions.len();

            for (i, den_pos) in den_positions.iter().enumerate() {
                let count = prey_per_den + if i < remainder { 1 } else { 0 };
                let mut spawned = 0;
                let mut attempts = 0;

                while spawned < count && attempts < count * 30 {
                    attempts += 1;
                    let dx = rng.random_range(-8..=8i32);
                    let dy = rng.random_range(-8..=8i32);
                    let x = (den_pos.x + dx).clamp(0, map_width - 1);
                    let y = (den_pos.y + dy).clamp(0, map_height - 1);
                    let terrain = terrain_at(x, y);
                    if seed.prey_habitat.contains(&terrain) {
                        prey_spawns.push((seed.kind, Position::new(x, y), den_start_idx + i));
                        spawned += 1;
                    }
                }
            }
        }
    }

    // Spawn den entities, collecting their Entity IDs for prey home_den.
    let den_entities: Vec<Entity> = den_spawns
        .into_iter()
        .map(|(den, pos)| world.spawn((den, Health::default(), pos)).id())
        .collect();

    // Spawn prey entities with home_den set.
    {
        let registry = world.resource::<SpeciesRegistry>();
        let bundles: Vec<(crate::components::prey::PreyBundle, Position, Entity)> = prey_spawns
            .iter()
            .map(|(kind, pos, den_idx)| {
                let profile = registry.find(*kind);
                let mut bundle = crate::components::prey::prey_bundle(profile);
                bundle.2.home_den = Some(den_entities[*den_idx]);
                (bundle, *pos, den_entities[*den_idx])
            })
            .collect();

        for (bundle, pos, _den_entity) in bundles {
            world.spawn((bundle, pos));
        }
    }

    // Run prey systems in a tight loop to simulate a mature ecosystem
    // before the cats arrive. No cats, no rendering — just prey breeding,
    // wandering, den refilling, and orphan prey founding new dens.
    presimulate_prey(world);
}

/// Run prey ecology for N ticks to let populations fill out and dens
/// spread organically before the main simulation starts.
fn presimulate_prey(world: &mut World) {
    use bevy_ecs::schedule::Schedule;

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            crate::systems::prey::prey_population,
            crate::systems::prey::prey_ai,
            crate::systems::prey::prey_den_lifecycle,
            crate::systems::prey::orphan_prey_adopt_or_found,
        )
            .chain(),
    );

    let presim_ticks = 5000; // ~5 sim days of prey-only ecology
    for _ in 0..presim_ticks {
        schedule.run(world);
    }

    // Count what we ended up with.
    let mut den_count = 0u32;
    let mut prey_count = 0u32;
    for _ in world.query::<&PreyDen>().iter(world) {
        den_count += 1;
    }
    for _ in world
        .query::<&crate::components::prey::PreyConfig>()
        .iter(world)
    {
        prey_count += 1;
    }
    eprintln!("  Prey presim: {presim_ticks} ticks → {den_count} dens, {prey_count} prey");
}
