//! Template coverage audit tool.
//!
//! Enumerates the combinatorial space of (Action, MoodBucket, Season, Weather)
//! and counts matching templates in the registry. Reports coverage gaps.
//!
//! Usage:
//!   cargo run --example template_audit
//!   just template-audit

use std::path::Path;

use clowder::ai::Action;
use clowder::components::identity::LifeStage;
use clowder::components::personality::Personality;
use clowder::components::physical::Needs;
use clowder::resources::map::Terrain;
use clowder::resources::narrative_templates::{MoodBucket, TemplateContext, TemplateRegistry};
use clowder::resources::time::{DayPhase, Season};
use clowder::resources::weather::Weather;

use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;

const ALL_ACTIONS: [Action; 26] = [
    Action::Eat,
    Action::Sleep,
    Action::Hunt,
    Action::Forage,
    Action::Wander,
    Action::Idle,
    Action::Socialize,
    Action::GroomSelf,
    Action::GroomOther,
    Action::Explore,
    Action::Flee,
    Action::Fight,
    Action::Patrol,
    Action::Build,
    Action::Farm,
    Action::HerbcraftGather,
    Action::HerbcraftRemedy,
    Action::HerbcraftSetWard,
    Action::MagicScry,
    Action::MagicDurableWard,
    Action::MagicCleanse,
    Action::MagicColonyCleanse,
    Action::MagicHarvest,
    Action::MagicCommune,
    Action::Coordinate,
    Action::Mentor,
];

/// Compile-time exhaustiveness witness: a new `Action` variant forces
/// the author to come here and decide whether it joins `ALL_ACTIONS`
/// (canonical template surface) or is intentionally excluded (Mate /
/// Caretake / Cook / Hide all alias to other files via the
/// `template_prompt` match block).
#[allow(dead_code)]
fn assert_all_actions_covers_action(a: Action) {
    match a {
        Action::Eat
        | Action::Sleep
        | Action::Hunt
        | Action::Forage
        | Action::Wander
        | Action::Idle
        | Action::Socialize
        | Action::GroomSelf
        | Action::GroomOther
        | Action::Explore
        | Action::Flee
        | Action::Fight
        | Action::Patrol
        | Action::Build
        | Action::Farm
        | Action::HerbcraftGather
        | Action::HerbcraftRemedy
        | Action::HerbcraftSetWard
        | Action::MagicScry
        | Action::MagicDurableWard
        | Action::MagicCleanse
        | Action::MagicColonyCleanse
        | Action::MagicHarvest
        | Action::MagicCommune
        | Action::Coordinate
        | Action::Mentor
        | Action::Mate
        | Action::Caretake
        | Action::Cook
        | Action::Hide => {}
    }
}

const ALL_MOODS: [MoodBucket; 5] = [
    MoodBucket::Miserable,
    MoodBucket::Low,
    MoodBucket::Neutral,
    MoodBucket::Happy,
    MoodBucket::Euphoric,
];

const ALL_SEASONS: [Season; 4] = [
    Season::Spring,
    Season::Summer,
    Season::Autumn,
    Season::Winter,
];

const ALL_WEATHER: [Weather; 8] = [
    Weather::Clear,
    Weather::Overcast,
    Weather::LightRain,
    Weather::HeavyRain,
    Weather::Snow,
    Weather::Fog,
    Weather::Wind,
    Weather::Storm,
];

fn main() {
    let template_path = Path::new("assets/narrative");
    let registry = match TemplateRegistry::load_from_dir(template_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to load templates: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "Loaded {} templates from {}",
        registry.len(),
        template_path.display()
    );
    println!();

    let mut rng = ChaCha8Rng::seed_from_u64(42);
    // Use a mid-range personality and default needs for matching.
    let personality = Personality::random(&mut rng);
    let needs = Needs::default();

    // Coverage matrix: Action × MoodBucket
    println!("Coverage by Action × Mood (matching template count):");
    println!(
        "{:<14} {:>8} {:>8} {:>8} {:>8} {:>8}  {:>5}",
        "Action", "Miser.", "Low", "Neutral", "Happy", "Euphor.", "Total"
    );
    println!("{}", "-".repeat(72));

    let mut total_cells = 0u32;
    let mut covered_cells = 0u32;
    let mut total_templates_matched = 0u32;

    for action in &ALL_ACTIONS {
        let mut row_total = 0u32;
        let mut cells = Vec::new();

        for mood in &ALL_MOODS {
            // Count matching templates across all seasons and weather for this (action, mood).
            let mut count = 0u32;
            for season in &ALL_SEASONS {
                for weather in &ALL_WEATHER {
                    let ctx = TemplateContext {
                        action: *action,
                        day_phase: DayPhase::Day,
                        season: *season,
                        weather: *weather,
                        mood_bucket: *mood,
                        life_stage: LifeStage::Adult,
                        has_target: false,
                        terrain: Terrain::Grass,
                        event: None,
                    };
                    if registry
                        .select(&ctx, &personality, &needs, &mut rng)
                        .is_some()
                    {
                        count += 1;
                    }
                }
            }
            total_cells += 1;
            if count > 0 {
                covered_cells += 1;
            }
            row_total += count;
            total_templates_matched += count;
            cells.push(count);
        }

        let label = format!("{:?}", action);
        println!(
            "{:<14} {:>8} {:>8} {:>8} {:>8} {:>8}  {:>5}",
            label, cells[0], cells[1], cells[2], cells[3], cells[4], row_total
        );
    }

    println!("{}", "-".repeat(72));
    let pct = (covered_cells as f64 / total_cells as f64) * 100.0;
    println!(
        "Coverage: {covered_cells}/{total_cells} cells ({pct:.0}%), {total_templates_matched} total matches"
    );

    // Highlight zero-coverage cells.
    println!();
    println!("Zero-coverage cells (Action × Mood with no matching template):");
    let mut gaps = 0u32;
    for action in &ALL_ACTIONS {
        for mood in &ALL_MOODS {
            let ctx = TemplateContext {
                action: *action,
                day_phase: DayPhase::Day,
                season: Season::Summer,
                weather: Weather::Clear,
                mood_bucket: *mood,
                life_stage: LifeStage::Adult,
                has_target: false,
                terrain: Terrain::Grass,
                event: None,
            };
            if registry
                .select(&ctx, &personality, &needs, &mut rng)
                .is_none()
            {
                println!("  {:?} × {}", action, mood.label());
                gaps += 1;
            }
        }
    }
    if gaps == 0 {
        println!("  (none — all Action × Mood cells have at least one template)");
    }
}
