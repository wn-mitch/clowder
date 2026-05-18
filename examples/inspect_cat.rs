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
    print_parental_vector(&cat_name, &snapshots);
    print_action_distribution(&cat_name, &actions, &snapshots);
    print_score_breakdown(&snapshots);
    print_needs_timeline(&snapshots);
    print_relationships(&snapshots);
    print_aspirations(&cat_name, &snapshots);
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
// Parental 5-vector (ticket 400)
// ---------------------------------------------------------------------------

fn print_parental_vector(name: &str, snapshots: &[Value]) {
    // Find the latest snapshot that carries a `parenting` block. Earlier
    // snapshots (before the cat became a parent) are skipped.
    let latest = snapshots
        .iter()
        .rev()
        .find_map(|s| s.get("parenting").and_then(|p| (!p.is_null()).then_some((s, p))));
    let Some((snap, p)) = latest else {
        return;
    };
    println!("=== {name} — Parental 5-vector (ticket 400) ===");
    println!();
    let tick = snap.get("tick").and_then(|t| t.as_u64()).unwrap_or(0);
    println!("As of tick {tick}:");
    let asymptote = field_f32(p, "asymptote");
    let engagement = field_f32(p, "parental_engagement_max");
    let suppression = field_f32(p, "caretake_suppression_factor");
    println!(
        "  engagement {engagement:.3} / asymptote {asymptote:.3} ({:.0}% converged)",
        if asymptote > 0.0 {
            (engagement / asymptote * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    );
    println!();
    println!("Five-scale composition (personality-derived):");
    for (label, key) in [
        ("Presence    ", "scale_presence"),
        ("Provision   ", "scale_provision"),
        ("Protection  ", "scale_protection"),
        ("Cultural    ", "scale_cultural"),
        ("Autonomy    ", "scale_autonomy"),
    ] {
        let v = field_f32(p, key);
        println!("  {label} {v:.2}  {}", make_bar(v, 20));
    }
    println!();
    println!("Per-DSE bias sums (lifts emitted by ParentingActivityModifier):");
    for (label, key) in [
        ("Caretake    ", "caretake_bias_sum"),
        ("Provision (Hunt) ", "provision_bias_sum"),
        ("Protect (Patrol)  ", "protect_bias_sum"),
        ("Cultural teach (Mentor)  ", "cultural_teach_bias_sum"),
        ("Autonomy teach (Mentor)  ", "autonomy_teach_bias_sum"),
    ] {
        let v = field_f32(p, key);
        println!("  {label} {v:.3}");
    }
    if suppression < 1.0 {
        println!();
        println!(
            "  JointIntention-aware suppression: ×{suppression:.2} on Caretake (partner is on it)"
        );
    }
    println!();
    let bio = field_u64(p, "biological_count");
    let inl = field_u64(p, "in_law_count");
    let bnd = field_u64(p, "bond_formed_count");
    let adp = field_u64(p, "adopted_count");
    println!(
        "Relationships: {bio} biological · {inl} in-law · {bnd} bond-formed · {adp} adopted"
    );
    println!();
}

fn field_f32(v: &Value, key: &str) -> f32 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32
}

fn field_u64(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
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
    sorted.sort_by_key(|b| std::cmp::Reverse(b.1));

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
                .is_some_and(|a| !a.is_empty())
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
        "temperature",
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
            // Thermal distress modifier (ticket 110) fires at
            // `thermal_deficit >= 0.7`, i.e. `temperature <= 0.3`.
            "temperature" => 0.3,
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
// Aspirations & Goal Stack (HTN substrate — ticket 336)
// ---------------------------------------------------------------------------

/// Renders the cat's aspiration set, current goal stack, and method history
/// derived from snapshot transitions. Reads `goal_stack` and
/// `active_aspirations` fields added to `CatSnapshot` by ticket 339; shows
/// a short notice if the run predates those fields or if no Tier-1 methods
/// were Live.
fn print_aspirations(name: &str, snapshots: &[Value]) {
    let has_aspirations = snapshots.iter().any(|s| {
        s.get("active_aspirations")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
    });
    let has_goal_stack = snapshots.iter().any(|s| {
        s.get("goal_stack")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
    });

    println!("=== {name} — Aspirations & Goal Stack ===");
    println!();

    if !has_aspirations && !has_goal_stack {
        println!("  (No HTN data in snapshots — Tier-1 methods not Live in this run)");
        println!();
        return;
    }

    // --- Active aspirations ---
    println!("  Active aspirations:");
    let last_asp = snapshots.iter().rev().find(|s| {
        s.get("active_aspirations")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
    });
    match last_asp {
        None => println!("    (none active)"),
        Some(snap) => {
            let tick = snap.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
            let asps = snap.get("active_aspirations").and_then(|v| v.as_array()).unwrap();
            println!("    (at tick {tick})");
            for (i, asp) in asps.iter().enumerate() {
                let chain = asp.get("chain_name").and_then(|v| v.as_str()).unwrap_or("?");
                let domain = asp.get("domain").and_then(|v| v.as_str()).unwrap_or("?");
                let milestone = asp.get("current_milestone").and_then(|v| v.as_u64()).unwrap_or(0);
                let progress = asp.get("progress").and_then(|v| v.as_u64()).unwrap_or(0);
                let adopted = asp.get("adopted_tick").and_then(|v| v.as_u64()).unwrap_or(0);
                println!(
                    "    [{i}] {chain:<24}  domain:{domain:<12}  milestone:{milestone}  progress:{progress:>4}  adopted:tick {adopted}"
                );
            }
        }
    }
    println!();

    // --- Current goal stack ---
    println!("  Goal stack (most recent):");
    let last_stack = snapshots.iter().rev().find(|s| {
        s.get("goal_stack")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
    });
    match last_stack {
        None => println!("    (stack empty — no active method)"),
        Some(snap) => {
            let tick = snap.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
            let frames = snap.get("goal_stack").and_then(|v| v.as_array()).unwrap();
            let current_action = snap.get("current_action").and_then(|v| v.as_str()).unwrap_or("?");
            println!("    (at tick {tick}  leaf action: {current_action})");
            for (depth, frame) in frames.iter().enumerate() {
                let method = frame.get("method").and_then(|v| v.as_str()).unwrap_or("?");
                let goal = frame.get("goal_label").and_then(|v| v.as_str()).unwrap_or("?");
                let sub_i = frame.get("sub_goal_index").and_then(|v| v.as_u64()).unwrap_or(0);
                let sub_n = frame.get("sub_goal_count").and_then(|v| v.as_u64()).unwrap_or(0);
                let source = frame.get("source").and_then(|v| v.as_str()).unwrap_or("?");
                let marker = if depth + 1 == frames.len() { " ← active" } else { "" };
                println!(
                    "    [{depth}] {method:<26}  goal:{goal:<20}  {sub_i}/{sub_n}  src:{source}{marker}"
                );
            }
        }
    }
    println!();

    // --- Method history derived from snapshot transitions ---
    //
    // MethodAdopted / SubGoalAdvanced / MethodBacktracked are Feature
    // activation counters, not EventKind log entries, so they are not
    // directly readable from events.jsonl. We infer them by comparing
    // consecutive snapshots: stack-top changes reveal adoptions and
    // backtracks; sub_goal_index increments reveal sub-goal advances.
    println!("  Method history (from snapshot transitions):");
    let history = derive_method_history(snapshots);
    if history.is_empty() {
        println!("    (no method transitions in snapshot window)");
    } else {
        let max_show = 20;
        let start = history.len().saturating_sub(max_show);
        if start > 0 {
            println!("    ... ({start} earlier events not shown)");
        }
        for (tick, event, detail) in &history[start..] {
            println!("    tick {:>7}  {:<24}  {}", tick, event, detail);
        }
    }
    println!();
}

/// Derives a coarse method-event timeline by diffing consecutive CatSnapshot
/// entries. Returns `(tick, event_label, detail)` triples in chronological
/// order. Only snapshots that carry a `goal_stack` field participate.
fn derive_method_history(snapshots: &[Value]) -> Vec<(u64, &'static str, String)> {
    let mut history: Vec<(u64, &'static str, String)> = Vec::new();
    let mut prev_method: Option<String> = None;
    let mut prev_sub_i: usize = 0;
    let mut prev_depth: usize = 0;

    for snap in snapshots {
        let frames = match snap.get("goal_stack").and_then(|v| v.as_array()) {
            Some(f) => f,
            None => continue,
        };
        let tick = snap.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
        let cur_depth = frames.len();
        let top = frames.last();
        let cur_method = top
            .and_then(|f| f.get("method"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let cur_sub_i = top
            .and_then(|f| f.get("sub_goal_index"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let sub_n = top
            .and_then(|f| f.get("sub_goal_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        match (&prev_method, &cur_method) {
            (None, Some(m)) => {
                history.push((tick, "MethodAdopted", m.clone()));
            }
            (Some(prev), None) => {
                history.push((tick, "MethodComplete/Abandoned", prev.clone()));
            }
            (Some(prev), Some(cur)) if cur != prev => {
                // Stack top changed to a different method.
                let prev_still_present = frames
                    .iter()
                    .any(|f| f.get("method").and_then(|v| v.as_str()) == Some(prev.as_str()));
                if !prev_still_present && cur_depth < prev_depth {
                    history.push((tick, "MethodBacktracked", format!("{prev} → {cur}")));
                } else {
                    history.push((tick, "MethodAdopted", cur.clone()));
                }
            }
            (Some(_), Some(cur)) => {
                // Same top method — check for sub-goal advance.
                if cur_sub_i > prev_sub_i {
                    history.push((
                        tick,
                        "SubGoalAdvanced",
                        format!("{cur} [{cur_sub_i}/{sub_n}]"),
                    ));
                }
            }
            _ => {}
        }

        prev_method = cur_method;
        prev_sub_i = cur_sub_i;
        prev_depth = cur_depth;
    }

    history
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
