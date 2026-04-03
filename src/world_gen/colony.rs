use bevy_ecs::prelude::World;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::components::building::Structure;
use crate::components::identity::Name;
use crate::components::{Appearance, Gender, Orientation, Personality, Position, Skills, ZodiacSign};
use crate::resources::map::{Terrain, TileMap};

/// A description of a cat to be spawned at game start.
///
/// The `position` field is initialised to `(0, 0)` and should be adjusted
/// relative to the colony site before spawning the entity.
pub struct CatBlueprint {
    pub name: String,
    pub gender: Gender,
    pub orientation: Orientation,
    pub personality: Personality,
    pub appearance: Appearance,
    pub skills: Skills,
    pub magic_affinity: f32,
    pub zodiac_sign: ZodiacSign,
    pub position: Position,
    /// Tick when this cat was born. Lower values = older cats.
    /// A `born_tick` of 0 at tick 0 means the cat is a newborn; negative
    /// offsets (stored as large u64) are not used — instead the cat's age is
    /// expressed as `current_tick - born_tick`.
    pub born_tick: u64,
}

// ---------------------------------------------------------------------------
// Colony site selection
// ---------------------------------------------------------------------------

/// Find a suitable colony site on `map`.
///
/// A valid site is:
/// - At least 15 tiles from any edge.
/// - The center tile is passable.
/// - More than 80 tiles in the 11×11 area around it are passable.
///
/// Tries up to 1 000 random positions. Falls back to map center if none qualify.
pub fn find_colony_site(map: &TileMap, rng: &mut impl Rng) -> Position {
    let margin = 15;
    let x_range = margin..(map.width - margin);
    let y_range = margin..(map.height - margin);

    // Guard: map must be large enough to have a valid search area.
    if x_range.is_empty() || y_range.is_empty() {
        return Position::new(map.width / 2, map.height / 2);
    }

    for _ in 0..1_000 {
        let cx: i32 = rng.random_range(x_range.clone());
        let cy: i32 = rng.random_range(y_range.clone());

        if !map.get(cx, cy).terrain.is_passable() {
            continue;
        }

        let passable_count = count_passable_in_area(map, cx, cy, 5);
        if passable_count > 80 {
            return Position::new(cx, cy);
        }
    }

    // Fallback: map center.
    Position::new(map.width / 2, map.height / 2)
}

/// Count passable tiles in the (2*radius+1) × (2*radius+1) area around (cx, cy).
fn count_passable_in_area(map: &TileMap, cx: i32, cy: i32, radius: i32) -> usize {
    let mut count = 0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = cx + dx;
            let y = cy + dy;
            if map.in_bounds(x, y) && map.get(x, y).terrain.is_passable() {
                count += 1;
            }
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Starting cat generation
// ---------------------------------------------------------------------------

const NAME_POOL: [&str; 30] = [
    "Bramble", "Thistle", "Cedar", "Moss", "Fern", "Ash", "Reed", "Clover", "Wren", "Hazel",
    "Rowan", "Sage", "Ivy", "Birch", "Flint", "Nettle", "Sorrel", "Briar", "Ember", "Willow",
    "Thorn", "Juniper", "Lark", "Pebble", "Lichen", "Mallow", "Basil", "Tansy", "Finch", "Heron",
];

const FUR_COLORS: [&str; 10] = [
    "ginger",
    "black",
    "white",
    "gray",
    "tabby brown",
    "calico",
    "tortoiseshell",
    "cream",
    "silver",
    "russet",
];

const EYE_COLORS: [&str; 6] = ["amber", "green", "blue", "copper", "hazel", "gold"];

const PATTERNS: [&str; 9] = [
    "solid", "tabby", "spotted", "tuxedo", "bicolor", "van", "point", "mackerel", "ticked",
];

/// Generate `count` starting cats with randomised attributes and varied ages.
///
/// `start_tick` is the simulation tick at which these cats are spawned.
/// Ages are distributed so that most cats are Young/Adult with ~1 Elder.
///
/// Names are unique (drawn without replacement from the pool).
/// `count` must not exceed 30.
pub fn generate_starting_cats(
    count: usize,
    start_tick: u64,
    ticks_per_season: u64,
    rng: &mut impl Rng,
) -> Vec<CatBlueprint> {
    assert!(count <= NAME_POOL.len(), "count exceeds name pool size");

    let mut names: Vec<&str> = NAME_POOL.to_vec();
    names.shuffle(rng);

    names
        .into_iter()
        .take(count)
        .map(|name| {
            let age_seasons = roll_age_seasons(rng);
            let age_ticks = age_seasons * ticks_per_season;
            let born_tick = start_tick.saturating_sub(age_ticks);
            generate_cat(name.to_string(), born_tick, ticks_per_season, rng)
        })
        .collect()
}

/// Roll an age in seasons with a bell curve weighted toward younger cats.
///
/// Distribution: ~60% Young (4-11), ~30% Adult (12-30), ~10% Elder (48-55).
fn roll_age_seasons(rng: &mut impl Rng) -> u64 {
    let roll: f32 = rng.random();
    if roll < 0.10 {
        // Elder: 48-55 seasons
        rng.random_range(48..=55)
    } else if roll < 0.40 {
        // Adult: 12-30 seasons
        rng.random_range(12..=30)
    } else {
        // Young: 4-11 seasons
        rng.random_range(4..=11)
    }
}

fn generate_cat(name: String, born_tick: u64, ticks_per_season: u64, rng: &mut impl Rng) -> CatBlueprint {
    let gender = roll_gender(rng);
    let orientation = roll_orientation(rng);
    let personality = Personality::random(rng);
    let magic_affinity = roll_magic_affinity(rng);
    let skills = roll_skills(&personality, magic_affinity, rng);
    let appearance = roll_appearance(rng);
    let birth_season = born_tick / ticks_per_season;
    let zodiac_sign = ZodiacSign::from_season(birth_season, rng);

    CatBlueprint {
        name,
        gender,
        orientation,
        personality,
        appearance,
        skills,
        magic_affinity,
        zodiac_sign,
        position: Position::new(0, 0),
        born_tick,
    }
}

/// ~50% Tom, ~45% Queen, ~5% Nonbinary.
fn roll_gender(rng: &mut impl Rng) -> Gender {
    match rng.random_range(0..20u32) {
        0..=9 => Gender::Tom,
        10..=18 => Gender::Queen,
        _ => Gender::Nonbinary,
    }
}

/// ~75% Straight, ~10% Gay, ~10% Bisexual, ~5% Asexual.
fn roll_orientation(rng: &mut impl Rng) -> Orientation {
    match rng.random_range(0..20u32) {
        0..=14 => Orientation::Straight,
        15..=16 => Orientation::Gay,
        17..=18 => Orientation::Bisexual,
        _ => Orientation::Asexual,
    }
}

/// 80% get 0.0–0.2, 15% get 0.3–0.6, 5% get 0.7–1.0.
fn roll_magic_affinity(rng: &mut impl Rng) -> f32 {
    match rng.random_range(0..20u32) {
        0..=15 => rng.random_range(0.0_f32..=0.2),
        16..=18 => rng.random_range(0.3_f32..=0.6),
        _ => rng.random_range(0.7_f32..=1.0),
    }
}

/// Start from default skills and add small personality-based aptitude boosts.
fn roll_skills(personality: &Personality, magic_affinity: f32, rng: &mut impl Rng) -> Skills {
    let _ = rng; // reserved for future randomised variance

    let mut skills = Skills::default();

    // Bold → better at hunting and combat.
    if personality.boldness > 0.6 {
        let boost = (personality.boldness - 0.6) * 0.5;
        skills.hunting += boost;
        skills.combat += boost;
    }

    // Diligent → better at building and foraging.
    if personality.diligence > 0.6 {
        let boost = (personality.diligence - 0.6) * 0.5;
        skills.building += boost;
        skills.foraging += boost;
    }

    // Spiritual + high magic affinity → magic aptitude.
    if personality.spirituality > 0.5 && magic_affinity > 0.2 {
        let boost = personality.spirituality * magic_affinity * 0.3;
        skills.magic += boost;
    }

    skills
}

fn roll_appearance(rng: &mut impl Rng) -> Appearance {
    let fur_color = FUR_COLORS[rng.random_range(0..FUR_COLORS.len())].to_string();
    let eye_color = EYE_COLORS[rng.random_range(0..EYE_COLORS.len())].to_string();
    let pattern = PATTERNS[rng.random_range(0..PATTERNS.len())].to_string();

    Appearance {
        fur_color,
        pattern,
        eye_color,
        distinguishing_marks: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Starting buildings
// ---------------------------------------------------------------------------

/// Stamp a building's full footprint onto the terrain map.
fn stamp_footprint(map: &mut TileMap, anchor: Position, terrain: Terrain, size: (i32, i32)) {
    for dy in 0..size.1 {
        for dx in 0..size.0 {
            let x = anchor.x + dx;
            let y = anchor.y + dy;
            if map.in_bounds(x, y) {
                map.set(x, y, terrain);
            }
        }
    }
}

/// Place starting building terrain tiles and spawn corresponding entities.
///
/// Creates a Hearth at the colony center, a Den three tiles west, and Stores
/// three tiles east. Each gets both a terrain footprint (for rendering) and an
/// ECS entity with `Structure` + `Position` + `Name` (for mechanical effects).
/// The anchor position is the top-left corner of the footprint.
pub fn spawn_starting_buildings(world: &mut World, colony_site: Position, map: &mut TileMap) {
    use crate::components::building::StructureType;

    let hearth_pos = colony_site;
    let den_size = StructureType::Den.default_size();
    let hearth_size = StructureType::Hearth.default_size();
    let stores_size = StructureType::Stores.default_size();
    // Space buildings so there's a 1-tile walkable gap between footprints.
    let den_pos = Position::new((colony_site.x - den_size.0 - 1).max(0), colony_site.y);
    let stores_pos = Position::new(
        (colony_site.x + hearth_size.0 + 1).min(map.width - stores_size.0),
        colony_site.y,
    );

    // Stamp full footprints for rendering.
    stamp_footprint(map, hearth_pos, Terrain::Hearth, hearth_size);
    stamp_footprint(map, den_pos, Terrain::Den, den_size);
    stamp_footprint(map, stores_pos, Terrain::Stores, stores_size);

    // Spawn building entities for mechanical effects.
    world.spawn((
        Name("The Hearth".to_string()),
        hearth_pos,
        Structure::new(StructureType::Hearth),
    ));
    world.spawn((
        Name("The Den".to_string()),
        den_pos,
        Structure::new(StructureType::Den),
    ));
    world.spawn((
        Name("The Stores".to_string()),
        stores_pos,
        Structure::new(StructureType::Stores),
    ));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_chacha::rand_core::SeedableRng;

    fn rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    fn grass_map(width: i32, height: i32) -> TileMap {
        TileMap::new(width, height, Terrain::Grass)
    }

    #[test]
    fn colony_site_is_passable() {
        let map = grass_map(100, 100);
        let pos = find_colony_site(&map, &mut rng(1));
        assert!(
            map.get(pos.x, pos.y).terrain.is_passable(),
            "colony site ({}, {}) is not passable",
            pos.x,
            pos.y
        );
    }

    #[test]
    fn colony_site_fallback_on_tiny_map() {
        // A 10×10 map is smaller than the 15-tile margin — should fall back to center.
        let map = grass_map(10, 10);
        let pos = find_colony_site(&map, &mut rng(1));
        assert_eq!(pos, Position::new(5, 5));
    }

    const START_TICK: u64 = 100_000;
    const TICKS_PER_SEASON: u64 = 2000;

    #[test]
    fn generate_starting_cats_correct_count() {
        let cats = generate_starting_cats(8, START_TICK, TICKS_PER_SEASON, &mut rng(42));
        assert_eq!(cats.len(), 8);
    }

    #[test]
    fn generated_cats_have_unique_names() {
        let cats = generate_starting_cats(10, START_TICK, TICKS_PER_SEASON, &mut rng(7));
        let mut names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10, "duplicate names found");
    }

    #[test]
    fn magic_affinity_in_range() {
        let mut r = rng(99);
        for _ in 0..50 {
            let cats = generate_starting_cats(5, START_TICK, TICKS_PER_SEASON, &mut r);
            for cat in &cats {
                assert!(
                    (0.0..=1.0).contains(&cat.magic_affinity),
                    "magic_affinity {} out of range",
                    cat.magic_affinity
                );
            }
        }
    }

    #[test]
    fn varied_starting_ages() {
        let cats = generate_starting_cats(8, START_TICK, TICKS_PER_SEASON, &mut rng(42));
        // All born_ticks should be <= start_tick (born in the past).
        for cat in &cats {
            assert!(
                cat.born_tick <= START_TICK,
                "born_tick {} should not exceed start_tick {}",
                cat.born_tick,
                START_TICK
            );
        }
        // At least two different born_ticks (ages should vary).
        let unique_ticks: std::collections::HashSet<u64> =
            cats.iter().map(|c| c.born_tick).collect();
        assert!(
            unique_ticks.len() > 1,
            "expected varied ages but all cats have the same born_tick"
        );
    }

    #[test]
    fn starting_buildings_fill_footprint() {
        use crate::components::building::StructureType;
        let mut map = grass_map(100, 100);
        let mut world = bevy_ecs::world::World::new();
        let center = Position::new(50, 50);
        spawn_starting_buildings(&mut world, center, &mut map);

        // Hearth at center, 2×2.
        let hearth_size = StructureType::Hearth.default_size();
        for dy in 0..hearth_size.1 {
            for dx in 0..hearth_size.0 {
                assert_eq!(
                    map.get(center.x + dx, center.y + dy).terrain,
                    Terrain::Hearth,
                    "Hearth tile at ({}, {}) not set",
                    center.x + dx,
                    center.y + dy,
                );
            }
        }

        // Den west of hearth, 2×2.
        let den_pos = Position::new(center.x - 2 - 1, center.y);
        let den_size = StructureType::Den.default_size();
        for dy in 0..den_size.1 {
            for dx in 0..den_size.0 {
                assert_eq!(
                    map.get(den_pos.x + dx, den_pos.y + dy).terrain,
                    Terrain::Den,
                    "Den tile at ({}, {}) not set",
                    den_pos.x + dx,
                    den_pos.y + dy,
                );
            }
        }

        // Stores east of hearth, 2×2.
        let stores_pos = Position::new(center.x + hearth_size.0 + 1, center.y);
        let stores_size = StructureType::Stores.default_size();
        for dy in 0..stores_size.1 {
            for dx in 0..stores_size.0 {
                assert_eq!(
                    map.get(stores_pos.x + dx, stores_pos.y + dy).terrain,
                    Terrain::Stores,
                    "Stores tile at ({}, {}) not set",
                    stores_pos.x + dx,
                    stores_pos.y + dy,
                );
            }
        }
    }
}
