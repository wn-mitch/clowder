use bevy_ecs::prelude::World;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::components::building::{ConstructionSite, Structure, StructureType};
use crate::components::identity::Name;
use crate::components::items::{BuildMaterialItem, Item, ItemKind, ItemLocation};
use crate::components::{
    Appearance, Gender, Orientation, Personality, Position, Skills, ZodiacSign,
};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::sim_constants::FounderAgeConstants;

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
/// Age distribution is controlled by `age_consts`.
///
/// Names are unique (drawn without replacement from the pool).
/// `count` must not exceed 30.
pub fn generate_starting_cats(
    count: usize,
    start_tick: u64,
    ticks_per_season: u64,
    age_consts: &FounderAgeConstants,
    rng: &mut impl Rng,
) -> Vec<CatBlueprint> {
    assert!(count <= NAME_POOL.len(), "count exceeds name pool size");

    let mut names: Vec<&str> = NAME_POOL.to_vec();
    names.shuffle(rng);

    names
        .into_iter()
        .take(count)
        .map(|name| {
            let age_seasons = roll_age_seasons(rng, age_consts);
            let age_ticks = age_seasons * ticks_per_season;
            let born_tick = start_tick.saturating_sub(age_ticks);
            generate_cat(name.to_string(), born_tick, ticks_per_season, rng)
        })
        .collect()
}

/// Roll an age in seasons from the configured founder-age distribution.
///
/// The default distribution is ~60% Young, ~30% Adult, ~10% senior-Adult —
/// the senior band stays below the Elder mortality ramp (see
/// `FounderAgeConstants`).
pub(crate) fn roll_age_seasons(rng: &mut impl Rng, consts: &FounderAgeConstants) -> u64 {
    let roll: f32 = rng.random();
    if roll < consts.young_probability {
        rng.random_range(consts.young_min_seasons..=consts.young_max_seasons)
    } else if roll < consts.young_probability + consts.adult_probability {
        rng.random_range(consts.adult_min_seasons..=consts.adult_max_seasons)
    } else {
        rng.random_range(consts.elder_min_seasons..=consts.elder_max_seasons)
    }
}

fn generate_cat(
    name: String,
    born_tick: u64,
    ticks_per_season: u64,
    rng: &mut impl Rng,
) -> CatBlueprint {
    let gender = roll_gender(rng);
    let orientation = roll_orientation(rng);
    let mut personality = Personality::random(rng);
    let appearance = roll_appearance(rng);
    apply_fur_color_bias(&mut personality, &appearance.fur_color, rng);
    let magic_affinity = roll_magic_affinity(rng);
    let skills = roll_skills(&personality, magic_affinity, rng);
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
pub(crate) fn roll_orientation(rng: &mut impl Rng) -> Orientation {
    match rng.random_range(0..20u32) {
        0..=14 => Orientation::Straight,
        15..=16 => Orientation::Gay,
        17..=18 => Orientation::Bisexual,
        _ => Orientation::Asexual,
    }
}

/// 60% get 0.0–0.2, 30% get 0.3–0.6, 10% get 0.7–1.0.
///
/// Rebalanced to raise the fraction of cats capable of crossing the
/// `magic_affinity_threshold` (0.3) for PracticeMagic — the prior 80/15/5
/// distribution produced ~1 magic-capable cat per 8-cat colony, which killed
/// the durable-ward path entirely (0/85 DurableWards placed in the baseline
/// 15-minute deep-soak).
pub(crate) fn roll_magic_affinity(rng: &mut impl Rng) -> f32 {
    match rng.random_range(0..20u32) {
        0..=11 => rng.random_range(0.0_f32..=0.2),
        12..=17 => rng.random_range(0.3_f32..=0.6),
        _ => rng.random_range(0.7_f32..=1.0),
    }
}

/// Start from default skills and add small personality-based aptitude boosts.
pub(crate) fn roll_skills(
    personality: &Personality,
    magic_affinity: f32,
    rng: &mut impl Rng,
) -> Skills {
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

    // Every cat is a latent magic user — a small random skill floor so
    // even low-affinity cats aren't mechanically zero.
    skills.magic = rng.random_range(0.0_f32..=0.1);

    // Cats with meaningful affinity roll an excellence multiplier. This is
    // the "some excel" knob: a per-cat random draw means two cats with the
    // same affinity will have different starting skill levels.
    if magic_affinity > 0.2 {
        skills.magic += magic_affinity * rng.random_range(0.3_f32..=1.0);
    }

    // Spiritual + high magic affinity adds an aptitude boost on top.
    if personality.spirituality > 0.5 && magic_affinity > 0.2 {
        let boost = personality.spirituality * magic_affinity * 0.3;
        skills.magic += boost;
    }

    skills.magic = skills.magic.clamp(0.0, 1.0);

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
// Fur-color personality biases
// ---------------------------------------------------------------------------

/// Shift personality axes based on fur color, adding per-trait jitter so
/// same-color cats aren't clones.
pub(crate) fn apply_fur_color_bias(
    personality: &mut Personality,
    fur_color: &str,
    rng: &mut impl Rng,
) {
    match fur_color {
        "ginger" => apply_ginger_bias(personality, rng),
        "calico" => apply_calico_bias(personality, rng),
        "black" => apply_black_bias(personality, rng),
        _ => {}
    }
}

/// Per-trait jitter so same-color cats aren't personality clones.
fn jitter(rng: &mut impl Rng) -> f32 {
    rng.random_range(-0.05_f32..0.05)
}

/// Ginger cats: dumber and crazier — bold, impulsive, short-tempered, fearless.
fn apply_ginger_bias(p: &mut Personality, rng: &mut impl Rng) {
    p.patience = (p.patience - 0.15 + jitter(rng)).clamp(0.0, 1.0);
    p.diligence = (p.diligence - 0.15 + jitter(rng)).clamp(0.0, 1.0);
    p.boldness = (p.boldness + 0.15 + jitter(rng)).clamp(0.0, 1.0);
    p.curiosity = (p.curiosity + 0.12 + jitter(rng)).clamp(0.0, 1.0);
    p.playfulness = (p.playfulness + 0.15 + jitter(rng)).clamp(0.0, 1.0);
    p.temper = (p.temper + 0.10 + jitter(rng)).clamp(0.0, 1.0);
    p.anxiety = (p.anxiety - 0.12 + jitter(rng)).clamp(0.0, 1.0);
}

/// Calico cats: demure — warm, patient, traditional, less aggressive.
fn apply_calico_bias(p: &mut Personality, rng: &mut impl Rng) {
    p.warmth = (p.warmth + 0.12 + jitter(rng)).clamp(0.0, 1.0);
    p.patience = (p.patience + 0.10 + jitter(rng)).clamp(0.0, 1.0);
    p.tradition = (p.tradition + 0.10 + jitter(rng)).clamp(0.0, 1.0);
    p.boldness = (p.boldness - 0.10 + jitter(rng)).clamp(0.0, 1.0);
    p.temper = (p.temper - 0.10 + jitter(rng)).clamp(0.0, 1.0);
    p.ambition = (p.ambition - 0.08 + jitter(rng)).clamp(0.0, 1.0);
}

/// Black cats: skittish and wiry — anxious, curious, independent, avoids crowds.
fn apply_black_bias(p: &mut Personality, rng: &mut impl Rng) {
    p.anxiety = (p.anxiety + 0.12 + jitter(rng)).clamp(0.0, 1.0);
    p.curiosity = (p.curiosity + 0.10 + jitter(rng)).clamp(0.0, 1.0);
    p.independence = (p.independence + 0.12 + jitter(rng)).clamp(0.0, 1.0);
    p.boldness = (p.boldness - 0.10 + jitter(rng)).clamp(0.0, 1.0);
    p.sociability = (p.sociability - 0.10 + jitter(rng)).clamp(0.0, 1.0);
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
/// Creates a Hearth at the colony center and a Den to its west, each separated
/// by a 1-tile walkable gap. The Stores building is NOT auto-spawned — it is
/// the colony's first real construction project, built organically when the
/// coordinator recognizes the need for food storage.
///
/// Starting food is scattered on the ground between the den and hearth,
/// representing supplies the cats brought with them (half lost in transit).
pub fn spawn_starting_buildings(world: &mut World, colony_site: Position, map: &mut TileMap) {
    let hearth_pos = colony_site;
    let den_size = StructureType::Den.default_size();
    let hearth_size = StructureType::Hearth.default_size();
    // Space buildings so there's a 1-tile walkable gap between footprints.
    let den_pos = Position::new((colony_site.x - den_size.0 - 1).max(0), colony_site.y);

    // Stamp full footprints for rendering.
    stamp_footprint(map, hearth_pos, Terrain::Hearth, hearth_size);
    stamp_footprint(map, den_pos, Terrain::Den, den_size);

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

    // Scatter starting food on the ground between den and hearth — the
    // supplies the colony carried with them. Ticket 038/041: bumped 15 → 35
    // so the founding wagon-dismantling haul cycle (`CLOWDER_FOUNDING_HAUL`)
    // doesn't push hunger over the edge in the early game while cats are
    // hauling Wood instead of hunting. Food still spoils and attracts
    // wildlife, preserving the build-a-store pressure.
    let starting_food: &[ItemKind] = &[
        ItemKind::RawRat,
        ItemKind::RawRat,
        ItemKind::RawRat,
        ItemKind::RawRat,
        ItemKind::RawRat,
        ItemKind::RawRat,
        ItemKind::RawFish,
        ItemKind::RawFish,
        ItemKind::RawFish,
        ItemKind::RawFish,
        ItemKind::RawFish,
        ItemKind::RawFish,
        ItemKind::RawBird,
        ItemKind::RawBird,
        ItemKind::RawBird,
        ItemKind::RawBird,
        ItemKind::RawMouse,
        ItemKind::RawMouse,
        ItemKind::RawMouse,
        ItemKind::RawMouse,
        ItemKind::RawMouse,
        ItemKind::RawRabbit,
        ItemKind::RawRabbit,
        ItemKind::RawRabbit,
        ItemKind::Berries,
        ItemKind::Berries,
        ItemKind::Berries,
        ItemKind::Berries,
        ItemKind::Nuts,
        ItemKind::Nuts,
        ItemKind::Nuts,
        ItemKind::Roots,
        ItemKind::Roots,
        ItemKind::Roots,
        ItemKind::WildOnion,
    ];

    // Scatter items in the walkable gap between den and hearth.
    let gap_x = den_pos.x + den_size.0; // The 1-tile gap column.
    let scatter_y_base = colony_site.y;
    for (i, &kind) in starting_food.iter().enumerate() {
        // Distribute items across a small area near the colony center.
        let dx = (i as i32) % 3;
        let dy = (i as i32) / 3;
        let food_pos = Position::new(
            (gap_x + dx).min(map.width - 1),
            (scatter_y_base + dy).min(map.height - 1),
        );
        world.spawn((Item::new(kind, 0.8, ItemLocation::OnGround), food_pos));
    }

    // Ticket 038 — founding wagon-dismantling spawn. Disabled by
    // default pending balance tuning: the haul cycle competes with
    // hunting/eating in the first few in-game days, pushing seed-42
    // starvation 0 → 5 in the canonical soak even with a small (4 Wood)
    // founding cost. The infrastructure (Pickup/Deliver step resolvers,
    // PlannerZone::MaterialPile, materials_available state, GOAP
    // dispatch) is wired and tested; only the spawn anchor is parked.
    // Activate via env var until a balance pass clears the regression.
    if std::env::var("CLOWDER_FOUNDING_HAUL").is_ok() {
        spawn_founding_construction_site(world, map, colony_site);
    }
}

/// Spawn the founding ConstructionSite plus its on-the-ground material
/// pile. The site uses a small custom Wood-only cost (4 Wood) so cats
/// finish the founding act in the first few in-game days without
/// starving while the long-term build economy comes online —
/// `resolve_construct` Fails on `!materials_complete()`. See ticket 038.
fn spawn_founding_construction_site(world: &mut World, map: &mut TileMap, colony_site: Position) {
    let blueprint = StructureType::Stores;
    let site_size = blueprint.default_size();

    // Place the site north of the colony center, leaving a 1-tile gap so
    // the south edge is reachable by cats spawned at colony_site.
    let site_anchor = Position::new(colony_site.x, (colony_site.y - site_size.1 - 1).max(0));

    // Stamp footprint terrain so the site renders correctly.
    stamp_footprint(map, site_anchor, blueprint.terrain(), site_size);

    // Founding cost — see fn doc. Smaller than the Stores blueprint
    // default (10 Wood + 5 Stone) so cats can clear the haul cycle
    // without sacrificing the early hunting / eating phase.
    let founding_cost = vec![(crate::components::task_chain::Material::Wood, 4u32)];
    let site = ConstructionSite::new_with_custom_cost(blueprint, founding_cost);
    let materials_needed = site.materials_needed.clone();

    // Spawn the construction site entity.
    world.spawn((
        Name(format!("Construction: {blueprint:?}")),
        site_anchor,
        Structure {
            kind: blueprint,
            condition: 0.0,
            cleanliness: 0.0,
            size: site_size,
        },
        site,
    ));

    // Spawn matching ground-item piles immediately south of the site
    // (between the colony spawn and the site, so cats encounter them on
    // the path to the site). Each unit becomes one Item entity — the
    // single-unit-per-pickup invariant keeps the cat→pile→site dance
    // physically honest.
    let pile_origin_x = site_anchor.x;
    let pile_origin_y = (site_anchor.y + site_size.1).min(map.height - 1);
    let mut spawned: i32 = 0;
    for (mat, count) in &materials_needed {
        let item_kind = match mat {
            crate::components::task_chain::Material::Wood => ItemKind::Wood,
            crate::components::task_chain::Material::Stone => ItemKind::Stone,
            crate::components::task_chain::Material::Herbs => continue, // not part of build economy
        };
        for _ in 0..*count {
            let dx = spawned % 3;
            let dy = spawned / 3;
            let pile_pos = Position::new(
                (pile_origin_x + dx).clamp(0, map.width - 1),
                (pile_origin_y + dy).clamp(0, map.height - 1),
            );
            world.spawn((
                Item::new(item_kind, 1.0, ItemLocation::OnGround),
                pile_pos,
                BuildMaterialItem,
            ));
            spawned += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::rand_core::SeedableRng;
    use rand_chacha::ChaCha8Rng;

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

    use crate::resources::time::TEST_TICKS_PER_SEASON as TICKS_PER_SEASON;
    const START_TICK: u64 = 200_000;

    fn age_consts() -> FounderAgeConstants {
        FounderAgeConstants::default()
    }

    #[test]
    fn generate_starting_cats_correct_count() {
        let cats =
            generate_starting_cats(8, START_TICK, TICKS_PER_SEASON, &age_consts(), &mut rng(42));
        assert_eq!(cats.len(), 8);
    }

    #[test]
    fn generated_cats_have_unique_names() {
        let cats =
            generate_starting_cats(10, START_TICK, TICKS_PER_SEASON, &age_consts(), &mut rng(7));
        let mut names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10, "duplicate names found");
    }

    #[test]
    fn magic_affinity_in_range() {
        let mut r = rng(99);
        let consts = age_consts();
        for _ in 0..50 {
            let cats = generate_starting_cats(5, START_TICK, TICKS_PER_SEASON, &consts, &mut r);
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
        let cats =
            generate_starting_cats(8, START_TICK, TICKS_PER_SEASON, &age_consts(), &mut rng(42));
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

    /// Invariant: founder ages must stay below the Elder mortality ramp.
    ///
    /// `elder_max_seasons` must be less than `DeathConstants::elder_entry_seasons +
    /// grace_seasons`. Breaking this reintroduces the pre-Activation-1 baseline
    /// wipe regression — see docs/balance/activation-1-status.md.
    #[test]
    fn founder_ages_leave_elder_grace() {
        use crate::resources::sim_constants::DeathConstants;
        let age_consts = FounderAgeConstants::default();
        let death = DeathConstants::default();
        let mortality_age = death.elder_entry_seasons + death.grace_seasons;
        let mut r = rng(42);
        for _ in 0..10_000 {
            let age = roll_age_seasons(&mut r, &age_consts);
            assert!(
                age < mortality_age,
                "founder rolled age {age} ≥ mortality age {mortality_age}; \
                 colonies will wipe before first breeding cycle",
            );
        }
    }

    #[test]
    fn ginger_cats_have_shifted_personality() {
        let mut r = rng(42);
        let mut bold_sum = 0.0f64;
        let mut patience_sum = 0.0f64;
        let n = 100;
        for _ in 0..n {
            let mut p = Personality::random(&mut r);
            apply_fur_color_bias(&mut p, "ginger", &mut r);
            bold_sum += p.boldness as f64;
            patience_sum += p.patience as f64;
        }
        let bold_mean = bold_sum / n as f64;
        let patience_mean = patience_sum / n as f64;
        assert!(
            bold_mean > 0.55,
            "ginger boldness mean {bold_mean} should be above 0.55"
        );
        assert!(
            patience_mean < 0.45,
            "ginger patience mean {patience_mean} should be below 0.45"
        );
    }

    #[test]
    fn calico_cats_are_demure() {
        let mut r = rng(42);
        let mut warmth_sum = 0.0f64;
        let mut boldness_sum = 0.0f64;
        let n = 100;
        for _ in 0..n {
            let mut p = Personality::random(&mut r);
            apply_fur_color_bias(&mut p, "calico", &mut r);
            warmth_sum += p.warmth as f64;
            boldness_sum += p.boldness as f64;
        }
        let warmth_mean = warmth_sum / n as f64;
        let boldness_mean = boldness_sum / n as f64;
        assert!(
            warmth_mean > 0.55,
            "calico warmth mean {warmth_mean} should be above 0.55"
        );
        assert!(
            boldness_mean < 0.45,
            "calico boldness mean {boldness_mean} should be below 0.45"
        );
    }

    #[test]
    fn black_cats_are_skittish() {
        let mut r = rng(42);
        let mut anxiety_sum = 0.0f64;
        let mut sociability_sum = 0.0f64;
        let n = 100;
        for _ in 0..n {
            let mut p = Personality::random(&mut r);
            apply_fur_color_bias(&mut p, "black", &mut r);
            anxiety_sum += p.anxiety as f64;
            sociability_sum += p.sociability as f64;
        }
        let anxiety_mean = anxiety_sum / n as f64;
        let sociability_mean = sociability_sum / n as f64;
        assert!(
            anxiety_mean > 0.55,
            "black anxiety mean {anxiety_mean} should be above 0.55"
        );
        assert!(
            sociability_mean < 0.45,
            "black sociability mean {sociability_mean} should be below 0.45"
        );
    }

    #[test]
    fn unbiased_colors_unaffected() {
        let mut r = rng(42);
        let p_before = Personality::random(&mut r);
        let mut p_after = p_before.clone();
        apply_fur_color_bias(&mut p_after, "gray", &mut r);
        assert_eq!(
            p_before, p_after,
            "gray cats should not have personality bias"
        );
    }

    #[test]
    fn starting_buildings_fill_footprint() {
        use crate::components::building::StructureType;
        let mut map = grass_map(120, 90);
        let mut world = bevy_ecs::world::World::new();
        let center = Position::new(60, 45);
        spawn_starting_buildings(&mut world, center, &mut map);

        let den_size = StructureType::Den.default_size();
        let hearth_size = StructureType::Hearth.default_size();

        // Derive positions the same way spawn_starting_buildings does.
        let hearth_pos = center;
        let den_pos = Position::new(center.x - den_size.0 - 1, center.y);

        // Hearth at center.
        for dy in 0..hearth_size.1 {
            for dx in 0..hearth_size.0 {
                assert_eq!(
                    map.get(hearth_pos.x + dx, hearth_pos.y + dy).terrain,
                    Terrain::Hearth,
                    "Hearth tile at ({}, {}) not set",
                    hearth_pos.x + dx,
                    hearth_pos.y + dy,
                );
            }
        }

        // Den west of hearth.
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

        // Stores is no longer auto-spawned — it's the colony's first
        // construction project. Verify its footprint is NOT stamped.
        let stores_size = StructureType::Stores.default_size();
        let stores_pos = Position::new(
            (center.x + hearth_size.0 + 1).min(map.width - stores_size.0),
            center.y,
        );
        assert_eq!(
            map.get(stores_pos.x, stores_pos.y).terrain,
            Terrain::Grass,
            "Stores should not be auto-spawned",
        );
    }

    #[test]
    fn starting_food_scattered_on_ground() {
        use crate::components::items::{Item, ItemLocation};

        let mut map = grass_map(120, 90);
        let mut world = bevy_ecs::world::World::new();
        let center = Position::new(60, 45);
        spawn_starting_buildings(&mut world, center, &mut map);

        // Count ground food items.
        let mut food_count = 0u32;
        let mut all_on_ground = true;
        for item in world.query::<&Item>().iter(&world) {
            if item.kind.is_food() {
                food_count += 1;
                if item.location != ItemLocation::OnGround {
                    all_on_ground = false;
                }
            }
        }

        assert_eq!(food_count, 35, "should scatter 35 starting food items");
        assert!(all_on_ground, "all starting food should be on the ground");
    }
}
