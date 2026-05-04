//! Ticket 162 — scenario harness CLI. Run a registered scenario by name
//! and print a per-tick decision-landscape report for the focal cat.
//!
//! Usage:
//!     just scenario <name> [--focal <cat>] [--ticks N] [--seed N]
//!     just scenario list
//!
//! See `src/scenarios/mod.rs::ALL` for the registered scenario list.

use clowder::scenarios;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_usage();
        std::process::exit(0);
    }

    if args[0] == "list" || args[0] == "--list" {
        print_scenario_list();
        return;
    }

    let scenario_name = args[0].clone();
    let mut focal: Option<String> = None;
    let mut ticks: Option<u32> = None;
    let mut seed: u64 = 42;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--focal" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--focal requires a value");
                    std::process::exit(2);
                }
                focal = Some(args[i].clone());
            }
            "--ticks" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--ticks requires a value");
                    std::process::exit(2);
                }
                ticks = Some(args[i].parse().unwrap_or_else(|_| {
                    eprintln!("--ticks must be a non-negative integer");
                    std::process::exit(2);
                }));
            }
            "--seed" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--seed requires a value");
                    std::process::exit(2);
                }
                seed = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("--seed must be a non-negative integer");
                    std::process::exit(2);
                });
            }
            other => {
                eprintln!("unknown arg: {other}");
                print_usage();
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let Some(scenario) = scenarios::by_name(&scenario_name) else {
        eprintln!("unknown scenario: {scenario_name}");
        print_scenario_list();
        std::process::exit(2);
    };

    let started = std::time::Instant::now();
    let report = scenarios::runner::run(scenario, focal.as_deref(), ticks, seed);
    let elapsed = started.elapsed();

    print_report(&report, elapsed);
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  scenario <name> [--focal <cat>] [--ticks N] [--seed N]");
    eprintln!("  scenario list");
    eprintln!();
    print_scenario_list();
}

fn print_scenario_list() {
    println!("registered scenarios:");
    for s in scenarios::ALL {
        println!(
            "  {:30}  default_focal={}, default_ticks={}",
            s.name, s.default_focal, s.default_ticks
        );
    }
}

fn print_report(report: &scenarios::runner::ScenarioReport, elapsed: std::time::Duration) {
    println!("─────────────────────────────────────────────────");
    println!("scenario: {}", report.scenario_name);
    println!("focal:    {}", report.focal);
    println!("seed:     {}", report.seed);
    println!("ticks:    {}", report.ticks.len());
    println!("elapsed:  {:.2}s", elapsed.as_secs_f64());
    println!("─────────────────────────────────────────────────");

    println!();
    println!("per-tick winner:");
    for t in &report.ticks {
        match &t.chosen {
            Some(c) => println!("  tick={:>10}  chose {}", t.tick, c),
            None => println!(
                "  tick={:>10}  (no L3 record — focal not yet resolved or held a multi-tick plan)",
                t.tick
            ),
        }
    }

    if let Some(last_tick) = report.ticks.iter().rev().find(|t| !t.ranked.is_empty()) {
        println!();
        println!(
            "softmax pool (tick={}, post-Independence penalty):",
            last_tick.tick
        );
        // Pair pool entries with their probabilities so the reader can
        // see "Hunt 0.62 → 91.2%" rather than scoring without context.
        // `softmax_probs` is parallel-indexed with `ranked` when the
        // softmax actually rolled; empty when the fallthrough path
        // (no eligible pool, or capture missing) was taken — in that
        // case we just omit the probability column.
        let probs_present = last_tick.softmax_probs.len() == last_tick.ranked.len();
        let mut paired: Vec<(String, f32, Option<f32>)> = last_tick
            .ranked
            .iter()
            .enumerate()
            .map(|(i, (name, score))| {
                (
                    name.clone(),
                    *score,
                    probs_present.then(|| last_tick.softmax_probs[i]),
                )
            })
            .collect();
        paired.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (name, score, prob) in &paired {
            match prob {
                Some(p) => println!("  {:25}  {:>8.4}   p={:>6.2}%", name, score, p * 100.0),
                None => println!("  {:25}  {:>8.4}   p=  —    ", name, score),
            }
        }
        if !probs_present {
            println!("  (softmax fallthrough — no rolled distribution; ranking from current.last_scores)");
        }
    }

    if let Some(last_tick) = report.ticks.iter().rev().find(|t| !t.l2.is_empty()) {
        println!();
        println!(
            "L2 score columns (tick={}, per-DSE, pre-Independence penalty):",
            last_tick.tick
        );
        println!(
            "  {:25}  {:>8}  {:>8}  {:>8}   modifiers",
            "dse", "raw", "gated", "final"
        );
        let mut sorted = last_tick.l2.clone();
        sorted.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for row in sorted {
            let mods = if row.modifier_deltas.is_empty() {
                "—".to_string()
            } else {
                row.modifier_deltas
                    .iter()
                    .map(|(name, value)| format!("{name}{value:+.3}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let elig = if row.eligible { "  " } else { "!!" };
            println!(
                "  {} {:23}  {:>8.4}  {:>8.4}  {:>8.4}   {}",
                elig, row.dse, row.raw_score, row.gated_score, row.final_score, mods
            );
        }
    }

    println!();
    println!("winner counts across run:");
    for (name, count) in report.winner_counts() {
        println!("  {:25}  {} ticks", name, count);
    }
    println!();
}
