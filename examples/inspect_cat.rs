//! Post-hoc cat inspection tool.
//!
//! Reads `logs/events.jsonl` (or a custom path) and prints a formatted report
//! for a named cat showing personality profile, action distribution, needs
//! timeline, relationships, key decisions, and death info.
//!
//! Usage: `cargo run --example inspect_cat -- <cat-name> [--events <path>]`

use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::PathBuf;

use serde_json::Value;

fn main() -> io::Result<()> {
    let (cat_name, events_path) = parse_args();

    let file = std::fs::File::open(&events_path).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("cannot open {}: {e}", events_path.display()),
        )
    })?;
    let reader = io::BufReader::new(file);

    let mut snapshots: Vec<Value> = Vec::new();
    let mut actions: Vec<Value> = Vec::new();
    let mut death: Option<Value> = None;

    for line in reader.lines() {
        let line = line?;
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let event_cat = v.get("cat").and_then(|c| c.as_str()).unwrap_or("");

        if event_cat != cat_name {
            continue;
        }

        match event_type {
            "CatSnapshot" => snapshots.push(v),
            "ActionChosen" => actions.push(v),
            "Death" => death = Some(v),
            _ => {}
        }
    }

    if snapshots.is_empty() && actions.is_empty() {
        eprintln!(
            "No events found for cat '{cat_name}' in {}",
            events_path.display()
        );
        eprintln!("Available cats:");
        // Re-scan for unique cat names.
        let file = std::fs::File::open(&events_path)?;
        let reader = io::BufReader::new(file);
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if let Some(name) = v.get("cat").and_then(|c| c.as_str()) {
                    names.insert(name.to_string());
                }
            }
        }
        for name in &names {
            eprintln!("  {name}");
        }
        return Ok(());
    }

    print_personality(&cat_name, &snapshots);
    print_action_distribution(&cat_name, &actions, &snapshots);
    print_score_breakdown(&snapshots);
    print_needs_timeline(&snapshots);
    print_relationships(&snapshots);
    print_key_decisions(&actions);
    if let Some(ref d) = death {
        print_death(d, &snapshots);
    }

    Ok(())
}

fn parse_args() -> (String, PathBuf) {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: inspect_cat <cat-name> [--events <path>]");
        std::process::exit(1);
    }
    let cat_name = args[1].clone();
    let mut events_path = PathBuf::from("logs/events.jsonl");
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--events" && i + 1 < args.len() {
            events_path = PathBuf::from(&args[i + 1]);
            i += 2;
        } else {
            i += 1;
        }
    }
    (cat_name, events_path)
}

// ---------------------------------------------------------------------------
// Personality Profile
// ---------------------------------------------------------------------------

fn print_personality(name: &str, snapshots: &[Value]) {
    let Some(snap) = snapshots.first() else {
        return;
    };
    let Some(p) = snap.get("personality") else {
        return;
    };

    println!("=== {name} — Personality Profile ===");
    println!();

    let drives = [
        ("boldness", "timid", "bold"),
        ("sociability", "reclusive", "sociable"),
        ("curiosity", "incurious", "curious"),
        ("diligence", "lazy", "diligent"),
        ("warmth", "aloof", "warm"),
        ("spirituality", "pragmatic", "spiritual"),
        ("ambition", "content", "ambitious"),
        ("patience", "impatient", "patient"),
    ];
    let temperament = [
        ("anxiety", "steady", "anxious"),
        ("optimism", "pessimistic", "optimistic"),
        ("temper", "calm", "hot-tempered"),
        ("stubbornness", "flexible", "stubborn"),
        ("playfulness", "serious", "playful"),
    ];
    let values = [
        ("loyalty", "fickle", "loyal"),
        ("tradition", "progressive", "traditional"),
        ("compassion", "callous", "compassionate"),
        ("pride", "humble", "proud"),
        ("independence", "dependent", "independent"),
    ];

    println!("  Drives:");
    print_axes(p, &drives);
    println!();
    println!("  Temperament:");
    print_axes(p, &temperament);
    println!();
    println!("  Values:");
    print_axes(p, &values);
    println!();
}

fn print_axes(personality: &Value, axes: &[(&str, &str, &str)]) {
    for (key, low_label, high_label) in axes {
        let val = personality
            .get(*key)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;
        let bar = make_bar(val, 10);
        let label = if val < 0.3 {
            *low_label
        } else if val > 0.7 {
            *high_label
        } else {
            "moderate"
        };
        println!("    {:<14} {:.2}  {}  {}", key, val, bar, label);
    }
}

fn make_bar(val: f32, width: usize) -> String {
    let filled = (val * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

// ---------------------------------------------------------------------------
// Action Distribution
// ---------------------------------------------------------------------------

fn print_action_distribution(name: &str, actions: &[Value], snapshots: &[Value]) {
    // Build counts from ActionChosen events if available, otherwise from snapshots.
    let mut counts: HashMap<String, usize> = HashMap::new();
    let source;
    if !actions.is_empty() {
        source = format!("{} decisions", actions.len());
        for a in actions {
            let action = a.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            *counts.entry(action.to_string()).or_default() += 1;
        }
    } else if !snapshots.is_empty() {
        source = format!("{} snapshots", snapshots.len());
        for s in snapshots {
            let action = s
                .get("current_action")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            *counts.entry(action.to_string()).or_default() += 1;
        }
    } else {
        return;
    }

    println!("=== {name} — Action Distribution ({source}) ===");
    println!();
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let max_count = sorted.first().map_or(1, |(_, c)| *c);
    let bar_max = 30;
    let total: f32 = sorted.iter().map(|(_, c)| *c).sum::<usize>() as f32;

    for (action, count) in &sorted {
        let pct = *count as f32 / total * 100.0;
        let bar_len = (*count as f32 / max_count as f32 * bar_max as f32) as usize;
        let bar = "\u{2588}".repeat(bar_len);
        println!("  {:<16} {:>5}  {}  {:.1}%", action, count, bar, pct);
    }

    // Personality correlation summary.
    if let Some(snap) = snapshots.first() {
        if let Some(p) = snap.get("personality") {
            println!();
            println!("  Personality correlations:");
            let boldness = p.get("boldness").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let diligence = p.get("diligence").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let curiosity = p.get("curiosity").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let sociability = p.get("sociability").and_then(|v| v.as_f64()).unwrap_or(0.5);

            let combat_pct = action_pct(&sorted, &["Hunt", "Fight", "Patrol"], total);
            let work_pct = action_pct(&sorted, &["Build", "Forage", "Farm"], total);
            let explore_pct = action_pct(&sorted, &["Explore", "Wander"], total);
            let social_pct = action_pct(&sorted, &["Socialize", "Groom"], total);

            println!(
                "    boldness={:.2}     -> combat-oriented: {:.1}%",
                boldness, combat_pct
            );
            println!(
                "    diligence={:.2}    -> work-oriented:   {:.1}%",
                diligence, work_pct
            );
            println!(
                "    curiosity={:.2}    -> exploration:     {:.1}%",
                curiosity, explore_pct
            );
            println!(
                "    sociability={:.2}  -> social:          {:.1}%",
                sociability, social_pct
            );
        }
    }
    println!();
}

fn action_pct(sorted: &[(String, usize)], actions: &[&str], total: f32) -> f32 {
    let sum: usize = sorted
        .iter()
        .filter(|(a, _)| actions.contains(&a.as_str()))
        .map(|(_, c)| c)
        .sum();
    sum as f32 / total * 100.0
}

// ---------------------------------------------------------------------------
// Score Breakdown
// ---------------------------------------------------------------------------

fn print_score_breakdown(snapshots: &[Value]) {
    // Collect snapshots that have last_scores data.
    let scored: Vec<&Value> = snapshots
        .iter()
        .filter(|s| {
            s.get("last_scores")
                .and_then(|v| v.as_array())
                .map_or(false, |a| !a.is_empty())
        })
        .collect();
    if scored.is_empty() {
        return;
    }

    println!(
        "=== Score Breakdown ({} snapshots with scores) ===",
        scored.len()
    );
    println!();

    // Flag Maslow violations: non-survival action won while hunger < 0.2.
    let mut violations = 0;
    for s in &scored {
        let hunger = s
            .get("needs")
            .and_then(|n| n.get("hunger"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        if hunger >= 0.2 {
            continue;
        }
        let scores = s.get("last_scores").and_then(|v| v.as_array()).unwrap();
        if let Some(top) = scores.first() {
            let action = top
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            if action != "Eat" && action != "Sleep" && action != "Flee" {
                violations += 1;
                if violations <= 3 {
                    let tick = s.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
                    let score = top
                        .as_array()
                        .and_then(|a| a.get(1))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    println!(
                        "  WARNING tick {tick}: {action} ({score:.2}) won over Eat while hunger={hunger:.2}"
                    );
                }
            }
        }
    }
    if violations > 3 {
        println!("  ... and {} more violations", violations - 3);
    }
    if violations == 0 {
        println!("  No Maslow violations detected (survival actions always won when hungry)");
    }

    // Show sample score breakdowns from a few evenly-spaced snapshots.
    let sample_count = 5.min(scored.len());
    let step = scored.len() / sample_count;
    println!();
    println!("  Sample scores:");
    for i in 0..sample_count {
        let s = scored[i * step];
        let tick = s.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
        let scores = s.get("last_scores").and_then(|v| v.as_array()).unwrap();
        let parts: Vec<String> = scores
            .iter()
            .filter_map(|entry| {
                let arr = entry.as_array()?;
                let action = arr.first()?.as_str()?;
                let score = arr.get(1)?.as_f64()?;
                Some(format!("{action} ({score:.2})"))
            })
            .collect();
        println!("    tick {:>7}: {}", tick, parts.join(" > "));
    }
    println!();
}

// ---------------------------------------------------------------------------
// Needs Timeline
// ---------------------------------------------------------------------------

fn print_needs_timeline(snapshots: &[Value]) {
    if snapshots.is_empty() {
        return;
    }

    println!("=== Needs Timeline ({} snapshots) ===", snapshots.len());
    println!();

    let need_keys = [
        "hunger",
        "energy",
        "warmth",
        "safety",
        "social",
        "acceptance",
        "respect",
        "mastery",
        "purpose",
    ];

    println!(
        "  {:<12} {:>6} {:>6} {:>6}  critical dips",
        "need", "min", "max", "final"
    );
    println!("  {}", "-".repeat(55));

    for key in &need_keys {
        let values: Vec<f32> = snapshots
            .iter()
            .filter_map(|s| s.get("needs")?.get(*key)?.as_f64().map(|v| v as f32))
            .collect();
        if values.is_empty() {
            continue;
        }
        let min = values.iter().copied().fold(f32::INFINITY, f32::min);
        let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let final_val = values.last().copied().unwrap_or(0.0);
        let critical_threshold = match *key {
            "hunger" | "energy" => 0.15,
            "safety" => 0.3,
            _ => 0.1,
        };
        let dips = values.iter().filter(|v| **v < critical_threshold).count();
        let dip_str = if dips > 0 {
            format!("{dips}")
        } else {
            "-".to_string()
        };
        println!(
            "  {:<12} {:>6.2} {:>6.2} {:>6.2}  {}",
            key, min, max, final_val, dip_str,
        );
    }
    println!();
}

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

fn print_relationships(snapshots: &[Value]) {
    let Some(snap) = snapshots.last() else { return };
    let Some(rels) = snap.get("relationships").and_then(|r| r.as_array()) else {
        return;
    };
    if rels.is_empty() {
        return;
    }

    println!("=== Relationships ===");
    println!();
    for rel in rels {
        let name = rel.get("cat").and_then(|c| c.as_str()).unwrap_or("?");
        let fondness = rel.get("fondness").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let familiarity = rel
            .get("familiarity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let bond = rel.get("bond").and_then(|b| b.as_str()).unwrap_or("");
        let bond_str = if bond.is_empty() {
            String::new()
        } else {
            format!("  [{bond}]")
        };
        println!(
            "  {:<12} fondness: {:+.2}  familiarity: {:.2}{}",
            name, fondness, familiarity, bond_str,
        );
    }
    println!();
}

// ---------------------------------------------------------------------------
// Key Decisions
// ---------------------------------------------------------------------------

fn print_key_decisions(actions: &[Value]) {
    if actions.is_empty() {
        return;
    }

    let last_n = 20;
    let start = actions.len().saturating_sub(last_n);
    let recent = &actions[start..];

    println!("=== Recent Decisions (last {}) ===", recent.len());
    println!();

    for a in recent {
        let tick = a.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
        let action = a.get("action").and_then(|v| v.as_str()).unwrap_or("?");
        let score = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let runner = a.get("runner_up").and_then(|v| v.as_str()).unwrap_or("?");
        let runner_score = a
            .get("runner_up_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let third = a.get("third").and_then(|v| v.as_str()).unwrap_or("?");
        let third_score = a.get("third_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        println!(
            "  tick {:>7}  {} ({:.2}) > {} ({:.2}) > {} ({:.2})",
            tick, action, score, runner, runner_score, third, third_score,
        );
    }
    println!();
}

// ---------------------------------------------------------------------------
// Death Report
// ---------------------------------------------------------------------------

fn print_death(death_event: &Value, snapshots: &[Value]) {
    let tick = death_event
        .get("tick")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cause = death_event
        .get("cause")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("=== Death ===");
    println!();
    println!("  Died at tick {tick} from {cause}");

    // Show final snapshots leading up to death.
    let last_snaps: Vec<&Value> = snapshots.iter().rev().take(3).collect();
    if !last_snaps.is_empty() {
        println!("  Final snapshots:");
        for s in last_snaps.iter().rev() {
            let snap_tick = s.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
            let needs = s.get("needs");
            let hunger = needs
                .and_then(|n| n.get("hunger"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let energy = needs
                .and_then(|n| n.get("energy"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let safety = needs
                .and_then(|n| n.get("safety"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let health = s.get("health").and_then(|v| v.as_f64()).unwrap_or(0.0);
            println!(
                "    tick {snap_tick}: hunger={hunger:.2} energy={energy:.2} safety={safety:.2} health={health:.2}"
            );
        }
    }
    println!();
}
