use rand::Rng;
use rand::seq::SliceRandom;

use crate::components::{Appearance, Gender, Orientation, Personality, Position, Skills};
use crate::resources::map::TileMap;

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
    pub position: Position,
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

/// Generate `count` starting cats with randomised attributes.
///
/// Names are unique (drawn without replacement from the pool).
/// `count` must not exceed 30.
pub fn generate_starting_cats(count: usize, rng: &mut impl Rng) -> Vec<CatBlueprint> {
    assert!(count <= NAME_POOL.len(), "count exceeds name pool size");

    let mut names: Vec<&str> = NAME_POOL.to_vec();
    names.shuffle(rng);

    names
        .into_iter()
        .take(count)
        .map(|name| generate_cat(name.to_string(), rng))
        .collect()
}

fn generate_cat(name: String, rng: &mut impl Rng) -> CatBlueprint {
    let gender = roll_gender(rng);
    let orientation = roll_orientation(rng);
    let personality = Personality::random(rng);
    let magic_affinity = roll_magic_affinity(rng);
    let skills = roll_skills(&personality, magic_affinity, rng);
    let appearance = roll_appearance(rng);

    CatBlueprint {
        name,
        gender,
        orientation,
        personality,
        appearance,
        skills,
        magic_affinity,
        position: Position::new(0, 0),
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::Terrain;
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

    #[test]
    fn generate_starting_cats_correct_count() {
        let cats = generate_starting_cats(8, &mut rng(42));
        assert_eq!(cats.len(), 8);
    }

    #[test]
    fn generated_cats_have_unique_names() {
        let cats = generate_starting_cats(10, &mut rng(7));
        let mut names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10, "duplicate names found");
    }

    #[test]
    fn magic_affinity_in_range() {
        let mut r = rng(99);
        for _ in 0..50 {
            let cats = generate_starting_cats(5, &mut r);
            for cat in &cats {
                assert!(
                    (0.0..=1.0).contains(&cat.magic_affinity),
                    "magic_affinity {} out of range",
                    cat.magic_affinity
                );
            }
        }
    }
}
