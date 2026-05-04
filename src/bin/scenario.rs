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

    if let Some(last_with_score) = report.ticks.iter().rev().find(|t| !t.ranked.is_empty()) {
        println!();
        println!("final-tick L2 ranked DSE table (tick={}):", last_with_score.tick);
        let mut sorted = last_with_score.ranked.clone();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (name, score) in sorted {
            println!("  {:25}  {:>8.4}", name, score);
        }
    }

    println!();
    println!("winner counts across run:");
    for (name, count) in report.winner_counts() {
        println!("  {:25}  {} ticks", name, count);
    }
    println!();
}
