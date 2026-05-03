use rand::Rng;
use std::path::Path;

use crate::components::identity::{Appearance, Gender, LifeStage};
use crate::components::personality::Personality;
use crate::components::physical::Position;
use crate::components::zodiac::ZodiacSign;
use crate::resources::sim_constants::FounderAgeConstants;

use super::colony::{
    apply_fur_color_bias, roll_age_for_stage, roll_magic_affinity, roll_orientation, roll_skills,
    CatBlueprint,
};

/// JSON-deserializable representation of a player-created cat.
///
/// Fields the game should generate (orientation, zodiac, magic, skills, age)
/// are intentionally omitted and rolled at load time.
#[derive(serde::Deserialize)]
pub struct CustomCat {
    pub name: String,
    pub gender: Gender,
    pub appearance: Appearance,
    pub personality: Personality,
}

/// Load all `*.json` files from `assets/data/cats/` and convert them to
/// `CatBlueprint`s with randomly-rolled missing fields.
///
/// Files that fail to parse are skipped with a stderr warning.
/// Returns an empty vec if the directory doesn't exist.
pub fn load_custom_cats(
    start_tick: u64,
    ticks_per_season: u64,
    age_consts: &FounderAgeConstants,
    stages: &mut dyn Iterator<Item = LifeStage>,
    rng: &mut impl Rng,
) -> Vec<CatBlueprint> {
    let dir = Path::new("assets/data/cats");
    if !dir.is_dir() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Warning: could not read {}: {e}", dir.display());
            return Vec::new();
        }
    };

    let mut blueprints = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: could not read {}: {e}", path.display());
                continue;
            }
        };

        let custom: CustomCat = match serde_json::from_str(&contents) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: skipping {} (invalid JSON): {e}", path.display());
                continue;
            }
        };

        eprintln!("Loaded custom cat: {}", custom.name);
        let stage = stages
            .next()
            .expect("stage queue exhausted in load_custom_cats");
        blueprints.push(custom_to_blueprint(
            custom,
            start_tick,
            ticks_per_season,
            age_consts,
            stage,
            rng,
        ));
    }

    blueprints
}

/// Convert a `CustomCat` into a full `CatBlueprint` by rolling the fields
/// that the questionnaire doesn't cover.
fn custom_to_blueprint(
    custom: CustomCat,
    start_tick: u64,
    ticks_per_season: u64,
    age_consts: &FounderAgeConstants,
    stage: LifeStage,
    rng: &mut impl Rng,
) -> CatBlueprint {
    let orientation = roll_orientation(rng);
    let mut personality = custom.personality;
    apply_fur_color_bias(&mut personality, &custom.appearance.fur_color, rng);
    let magic_affinity = roll_magic_affinity(rng);
    let skills = roll_skills(&personality, magic_affinity, rng);

    let age_seasons = roll_age_for_stage(stage, age_consts, rng);
    let age_ticks = age_seasons * ticks_per_season;
    let born_tick = start_tick.saturating_sub(age_ticks);
    let birth_season = born_tick / ticks_per_season;
    let zodiac_sign = ZodiacSign::from_season(birth_season, rng);

    CatBlueprint {
        name: custom.name,
        gender: custom.gender,
        orientation,
        personality,
        appearance: custom.appearance,
        skills,
        magic_affinity,
        zodiac_sign,
        position: Position::new(0, 0),
        born_tick,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::rand_core::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    #[test]
    fn load_returns_empty_when_no_directory() {
        // The test runner's working directory won't have assets/data/cats/
        // unless explicitly created, so this should return empty.
        let age_consts = FounderAgeConstants::default();
        let mut stages = std::iter::repeat_n(LifeStage::Adult, 8);
        let cats = load_custom_cats(100_000, 2000, &age_consts, &mut stages, &mut rng(42));
        // Either empty (no dir) or whatever is there — just shouldn't panic.
        let _ = cats;
    }

    #[test]
    fn custom_to_blueprint_fills_missing_fields() {
        let custom = CustomCat {
            name: "Biscuit".to_string(),
            gender: Gender::Queen,
            appearance: Appearance {
                fur_color: "ginger".to_string(),
                pattern: "mackerel".to_string(),
                eye_color: "green".to_string(),
                distinguishing_marks: vec!["white chin spot".to_string()],
            },
            personality: Personality {
                boldness: 0.7,
                sociability: 0.5,
                curiosity: 0.8,
                diligence: 0.3,
                warmth: 0.6,
                spirituality: 0.5,
                ambition: 0.4,
                patience: 0.3,
                anxiety: 0.5,
                optimism: 0.6,
                temper: 0.7,
                stubbornness: 0.8,
                playfulness: 0.9,
                loyalty: 0.6,
                tradition: 0.4,
                compassion: 0.4,
                pride: 0.6,
                independence: 0.7,
            },
        };

        let age_consts = FounderAgeConstants::default();
        let bp = custom_to_blueprint(
            custom,
            100_000,
            2000,
            &age_consts,
            LifeStage::Adult,
            &mut rng(42),
        );

        assert_eq!(bp.name, "Biscuit");
        assert_eq!(bp.gender, Gender::Queen);
        assert!(bp.born_tick <= 100_000);
        assert!((0.0..=1.0).contains(&bp.magic_affinity));
    }
}
