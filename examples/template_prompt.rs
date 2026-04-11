//! Interactive narrative template authoring tool.
//!
//! Generates random scenarios (action + conditions), prompts you for a
//! template sentence, shows a live preview with variables resolved, and
//! appends the confirmed RON entry to the correct assets file.
//!
//! Usage:
//!   cargo run --example template_prompt
//!   just template-prompt

use std::io::{self, BufRead, Write};

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

use clowder::ai::Action;
use clowder::components::identity::{Gender, LifeStage};
use clowder::resources::narrative_templates::{
    MoodBucket, NeedAxis, NeedLevel, PersonalityAxis, PersonalityBucket, VariableContext,
    resolve_variables,
};
use clowder::resources::time::{DayPhase, Season};
use clowder::resources::weather::Weather;

const PREVIEW_NAMES: [&str; 6] = ["Bramble", "Thistle", "Moss", "Fern", "Ash", "Reed"];
const PREVIEW_FUR: [&str; 5] = ["tabby", "black", "tortoiseshell", "ginger", "grey"];
const PREVIEW_GENDERS: [Gender; 3] = [Gender::Tom, Gender::Queen, Gender::Nonbinary];

fn main() {
    let seed: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let stdin = io::stdin();

    println!("Narrative template author. Enter text, or:");
    println!("  (empty) skip    q/quit  exit    Ctrl-D  exit");
    println!();

    loop {
        let action = pick_action(&mut rng);
        let conditions = pick_conditions(&mut rng);

        let ron_file = match action {
            Action::Eat => "eat.ron",
            Action::Sleep => "sleep.ron",
            Action::Hunt => "hunt.ron",
            Action::Forage => "forage.ron",
            Action::Wander => "wander.ron",
            Action::Idle => "idle.ron",
            Action::Socialize => "socialize.ron",
            Action::Groom => "groom.ron",
            Action::Explore => "explore.ron",
            Action::Flee => "flee.ron",
            Action::Fight => "fight.ron",
            Action::Patrol => "patrol.ron",
            Action::Build => "build.ron",
            Action::Farm => "farm.ron",
            Action::Herbcraft => "herbcraft.ron",
            Action::PracticeMagic => "magic.ron",
            Action::Coordinate => "coordinate.ron",
            Action::Mentor => "mentor.ron",
            Action::Mate => "socialize.ron",
            Action::Caretake => "socialize.ron",
        };

        let tier = match action {
            Action::Idle => "Micro",
            _ => "Action",
        };

        // Build a preview context from the rolled conditions.
        let preview_name = PREVIEW_NAMES[rng.random_range(0..PREVIEW_NAMES.len())];
        let preview_gender = PREVIEW_GENDERS[rng.random_range(0..PREVIEW_GENDERS.len())];
        let preview_fur = PREVIEW_FUR[rng.random_range(0..PREVIEW_FUR.len())];

        let preview_weather = conditions.iter().find_map(|c| match c {
            Condition::Weather(w) => Some(*w),
            _ => None,
        }).unwrap_or(Weather::Clear);

        let preview_day_phase = conditions.iter().find_map(|c| match c {
            Condition::DayPhase(dp) => Some(*dp),
            _ => None,
        }).unwrap_or(DayPhase::Day);

        let preview_season = conditions.iter().find_map(|c| match c {
            Condition::Season(s) => Some(*s),
            _ => None,
        }).unwrap_or(Season::Summer);

        let preview_life_stage = conditions.iter().find_map(|c| match c {
            Condition::LifeStage(ls) => Some(*ls),
            _ => None,
        }).unwrap_or(LifeStage::Adult);

        let var_ctx = VariableContext {
            name: preview_name,
            gender: preview_gender,
            weather: preview_weather,
            day_phase: preview_day_phase,
            season: preview_season,
            life_stage: preview_life_stage,
            fur_color: preview_fur,
            other: None,
            prey: None,
            item: None,
            quality: None,
        };

        // Print the scenario.
        println!("── Template Prompt ──────────────────────────");
        println!("Action:      {:?}", action);
        for cond in &conditions {
            println!("{}", cond);
        }
        println!();
        println!("Variables:   {{name}}, {{subject}}, {{object}}, {{possessive}}, {{Subject}},");
        println!("             {{weather_desc}}, {{time_desc}}, {{season}}, {{fur_color}}");
        println!(
            "Preview as:  {} ({}, {}, {})",
            preview_name,
            preview_gender.subject_pronoun(),
            preview_fur,
            preview_life_stage_label(preview_life_stage),
        );
        println!("─────────────────────────────────────────────");

        // Input → preview → confirm loop for this scenario.
        loop {
            print!("> ");
            io::stdout().flush().unwrap();

            let mut line = String::new();
            let bytes = stdin.lock().read_line(&mut line).unwrap_or(0);
            if bytes == 0 {
                println!();
                return;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                println!("(skipped)\n");
                break;
            }
            if trimmed == "q" || trimmed == "quit" {
                return;
            }

            // Reject control characters (e.g. stray ESC sequences).
            if trimmed.chars().any(|c| c.is_control()) {
                println!("  (contains control characters — discarded)\n");
                break;
            }

            // Show preview.
            let preview = resolve_variables(trimmed, &var_ctx);
            println!();
            println!("  Preview:  {}", preview);
            println!();

            // Confirm.
            print!("  Save? [y]es / [n]o / [e]dit > ");
            io::stdout().flush().unwrap();

            let mut confirm = String::new();
            let confirm_bytes = stdin.lock().read_line(&mut confirm).unwrap_or(0);
            if confirm_bytes == 0 {
                println!();
                return;
            }

            let choice = confirm.trim().to_lowercase();
            if choice == "n" || choice == "no" {
                println!("  (discarded)\n");
                break;
            }
            if choice == "e" || choice == "edit" {
                // Re-prompt for the same scenario.
                continue;
            }

            // Default (Enter, "y", "yes") → save.
            let mut entry = String::new();
            entry.push_str("    (\n");
            let escaped = trimmed.replace('\\', "\\\\").replace('"', "\\\"");
            entry.push_str(&format!("        text: \"{}\",\n", escaped));
            entry.push_str(&format!("        tier: {},\n", tier));
            entry.push_str(&format!("        action: Some({:?}),\n", action));
            for cond in &conditions {
                entry.push_str(&format!("{}\n", cond.ron_field()));
            }
            entry.push_str("    ),\n");

            let path = format!("assets/narrative/{}", ron_file);
            match append_to_ron_file(&path, &entry) {
                Ok(count) => {
                    println!("  Added to {}. ({} templates total)\n", ron_file, count);
                }
                Err(e) => {
                    eprintln!("  Error writing to {}: {}\n", path, e);
                }
            }
            break;
        }
    }
}

fn preview_life_stage_label(ls: LifeStage) -> &'static str {
    match ls {
        LifeStage::Kitten => "kitten",
        LifeStage::Young => "young",
        LifeStage::Adult => "adult",
        LifeStage::Elder => "elder",
    }
}

/// Insert `entry` before the closing `]` in a RON list file.
/// Returns the total number of templates in the file after insertion.
fn append_to_ron_file(path: &str, entry: &str) -> io::Result<usize> {
    let contents = std::fs::read_to_string(path)?;

    let close_pos = contents
        .rfind(']')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no closing ] in RON file"))?;

    let mut new_contents = String::with_capacity(contents.len() + entry.len() + 2);
    let before = &contents[..close_pos];

    new_contents.push_str(before);
    if !before.ends_with('\n') {
        new_contents.push('\n');
    }
    new_contents.push_str(entry);
    new_contents.push_str(&contents[close_pos..]);

    std::fs::write(path, &new_contents)?;

    let count = new_contents.matches("text:").count();
    Ok(count)
}

// ---------------------------------------------------------------------------
// Condition types
// ---------------------------------------------------------------------------

enum Condition {
    Weather(Weather),
    DayPhase(DayPhase),
    Season(Season),
    Mood(MoodBucket),
    Personality(PersonalityAxis, PersonalityBucket),
    Need(NeedAxis, NeedLevel),
    LifeStage(LifeStage),
}

impl std::fmt::Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Condition::Weather(w) => write!(f, "Weather:     {}", w.label()),
            Condition::DayPhase(dp) => write!(f, "Day Phase:   {}", dp.label()),
            Condition::Season(s) => write!(f, "Season:      {:?}", s),
            Condition::Mood(m) => write!(f, "Mood:        {}", m.label()),
            Condition::Personality(axis, bucket) => {
                let desc = match bucket {
                    PersonalityBucket::Low => axis.low_label().to_string(),
                    PersonalityBucket::Mid => {
                        format!("between {} and {}", axis.low_label(), axis.high_label())
                    }
                    PersonalityBucket::High => axis.high_label().to_string(),
                };
                write!(
                    f,
                    "Personality: {} = {} ({})",
                    axis.label(),
                    bucket.label(),
                    desc
                )
            }
            Condition::Need(axis, level) => {
                let desc = match level {
                    NeedLevel::Critical => "desperately low",
                    NeedLevel::Low => "running low",
                    NeedLevel::Moderate => "adequate",
                    NeedLevel::Satisfied => "fully satisfied",
                };
                write!(f, "Need:        {} — {}", axis.label(), desc)
            }
            Condition::LifeStage(ls) => write!(f, "Life Stage:  {:?}", ls),
        }
    }
}

impl Condition {
    fn ron_field(&self) -> String {
        match self {
            Condition::Weather(w) => format!("        weather: Some({:?}),", w),
            Condition::DayPhase(dp) => format!("        day_phase: Some({:?}),", dp),
            Condition::Season(s) => format!("        season: Some({:?}),", s),
            Condition::Mood(m) => format!("        mood: Some({:?}),", m),
            Condition::Personality(axis, bucket) => {
                format!(
                    "        personality: [(axis: {:?}, bucket: {:?})],",
                    axis, bucket
                )
            }
            Condition::Need(axis, level) => {
                format!(
                    "        needs: [(axis: {:?}, level: {:?})],",
                    axis, level
                )
            }
            Condition::LifeStage(ls) => format!("        life_stage: Some({:?}),", ls),
        }
    }
}

// ---------------------------------------------------------------------------
// Random pickers
// ---------------------------------------------------------------------------

fn pick_action(rng: &mut impl Rng) -> Action {
    match rng.random_range(0..18) {
        0 => Action::Eat,
        1 => Action::Sleep,
        2 => Action::Hunt,
        3 => Action::Forage,
        4 => Action::Wander,
        5 => Action::Idle,
        6 => Action::Socialize,
        7 => Action::Groom,
        8 => Action::Explore,
        9 => Action::Flee,
        10 => Action::Fight,
        11 => Action::Patrol,
        12 => Action::Build,
        13 => Action::Farm,
        14 => Action::Herbcraft,
        15 => Action::PracticeMagic,
        16 => Action::Coordinate,
        _ => Action::Mentor,
    }
}

fn pick_conditions(rng: &mut impl Rng) -> Vec<Condition> {
    let count = rng.random_range(1..=3);
    let mut conditions = Vec::new();
    let mut used_types = Vec::new();

    for _ in 0..count {
        let mut kind = rng.random_range(0..7);
        let mut attempts = 0;
        while used_types.contains(&kind) && attempts < 20 {
            kind = rng.random_range(0..7);
            attempts += 1;
        }
        if used_types.contains(&kind) {
            continue;
        }
        used_types.push(kind);

        let cond = match kind {
            0 => {
                let weathers = [
                    Weather::Clear,
                    Weather::Overcast,
                    Weather::LightRain,
                    Weather::HeavyRain,
                    Weather::Snow,
                    Weather::Fog,
                    Weather::Wind,
                    Weather::Storm,
                ];
                Condition::Weather(weathers[rng.random_range(0..weathers.len())])
            }
            1 => {
                let phases = [DayPhase::Dawn, DayPhase::Day, DayPhase::Dusk, DayPhase::Night];
                Condition::DayPhase(phases[rng.random_range(0..phases.len())])
            }
            2 => {
                let seasons =
                    [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];
                Condition::Season(seasons[rng.random_range(0..seasons.len())])
            }
            3 => {
                let moods = [
                    MoodBucket::Miserable,
                    MoodBucket::Low,
                    MoodBucket::Neutral,
                    MoodBucket::Happy,
                    MoodBucket::Euphoric,
                ];
                Condition::Mood(moods[rng.random_range(0..moods.len())])
            }
            4 => {
                let axis = PersonalityAxis::ALL[rng.random_range(0..18)];
                let bucket = match rng.random_range(0..4) {
                    0 => PersonalityBucket::Low,
                    1 => PersonalityBucket::Mid,
                    _ => PersonalityBucket::High,
                };
                Condition::Personality(axis, bucket)
            }
            5 => {
                let axis = NeedAxis::ALL[rng.random_range(0..9)];
                let level = match rng.random_range(0..4) {
                    0 => NeedLevel::Critical,
                    1 => NeedLevel::Low,
                    2 => NeedLevel::Moderate,
                    _ => NeedLevel::Satisfied,
                };
                Condition::Need(axis, level)
            }
            _ => {
                let stages = [
                    LifeStage::Kitten,
                    LifeStage::Young,
                    LifeStage::Adult,
                    LifeStage::Elder,
                ];
                Condition::LifeStage(stages[rng.random_range(0..stages.len())])
            }
        };
        conditions.push(cond);
    }

    conditions
}
