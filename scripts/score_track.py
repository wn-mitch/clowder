#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# ///
"""
Score tracker for Clowder simulation.

Runs headless benchmarks across multiple seeds, extracts final ColonyScore
and SystemActivation data, and appends a row to logs/score_history.jsonl
tagged with the current jj changeset.

Usage:
    uv run scripts/score_track.py [--duration 15] [--seeds 42,99,7,2025,314]
"""

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path


def get_vcs_info() -> dict:
    """Get current jj change-id, git commit hash, and description."""
    try:
        result = subprocess.run(
            ["jj", "log", "--no-pager", "--no-graph", "-r", "@",
             "--template", 'change_id.short() ++ "\\n" ++ commit_id.short() ++ "\\n" ++ description.first_line()'],
            capture_output=True, text=True, check=True,
        )
        lines = [l for l in result.stdout.strip().splitlines() if l.strip()]
        change_id = lines[0].strip() if len(lines) > 0 else "unknown"
        commit_hash = lines[1].strip() if len(lines) > 1 else "unknown"
        description = lines[2].strip() if len(lines) > 2 else "(no description)"
        return {
            "change_id": change_id,
            "commit_hash": commit_hash,
            "description": description,
        }
    except (subprocess.CalledProcessError, FileNotFoundError):
        return {
            "change_id": "unknown",
            "commit_hash": "unknown",
            "description": "(jj not available)",
        }


def build_release() -> bool:
    """Build in release mode for consistent timing."""
    print("Building release binary...", end=" ", flush=True)
    result = subprocess.run(
        ["cargo", "build", "--release", "--quiet"],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        print("FAILED")
        print(result.stderr, file=sys.stderr)
        return False
    print("ok")
    return True


def run_seed(seed: int, duration: int) -> dict | None:
    """Run a headless simulation and extract final scores."""
    with tempfile.TemporaryDirectory() as tmpdir:
        event_path = Path(tmpdir) / "events.jsonl"
        log_path = Path(tmpdir) / "narrative.jsonl"

        result = subprocess.run(
            ["cargo", "run", "--release", "--quiet", "--",
             "--headless",
             "--duration", str(duration),
             "--seed", str(seed),
             "--event-log", str(event_path),
             "--log", str(log_path)],
            capture_output=True, text=True,
        )

        if result.returncode != 0:
            print(f"  seed {seed}: FAILED ({result.stderr.strip()[:80]})")
            return None

        if not event_path.exists():
            print(f"  seed {seed}: no event log produced")
            return None

        # Parse events — find last ColonyScore, last SystemActivation, and header
        header = None
        last_score = None
        last_activation = None

        with open(event_path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                obj = json.loads(line)
                if obj.get("_header"):
                    header = obj
                    continue
                event_type = obj.get("type")
                if event_type == "ColonyScore":
                    last_score = obj
                elif event_type == "SystemActivation":
                    last_activation = obj

        if last_score is None:
            print(f"  seed {seed}: no ColonyScore event")
            return None

        # Extract constants hash from header
        constants_hash = None
        if header and "constants" in header:
            constants_json = json.dumps(header["constants"], sort_keys=True)
            constants_hash = hashlib.sha256(constants_json.encode()).hexdigest()[:16]

        # Build per-seed summary
        seed_data = {
            "welfare": round(last_score.get("welfare", 0), 4),
            "aggregate": round(last_score.get("aggregate", 0), 2),
            "activation_score": round(last_score.get("activation_score", 0), 2),
            "features_active": last_score.get("features_active", 0),
            "features_total": last_score.get("features_total", 0),
            "deaths_starvation": last_score.get("deaths_starvation", 0),
            "deaths_old_age": last_score.get("deaths_old_age", 0),
            "deaths_injury": last_score.get("deaths_injury", 0),
            "living_cats": last_score.get("living_cats", 0),
            "seasons_survived": last_score.get("seasons_survived", 0),
            "bonds_formed": last_score.get("bonds_formed", 0),
            "aspirations_completed": last_score.get("aspirations_completed", 0),
        }

        # Add dead features list from activation data
        if last_activation:
            counts = last_activation.get("counts", {})
            seed_data["dead_features"] = sorted(
                k for k, v in counts.items() if v == 0
            )

        return {
            "seed_data": seed_data,
            "constants_hash": constants_hash,
        }


def main():
    parser = argparse.ArgumentParser(description="Clowder score tracker")
    parser.add_argument("--duration", type=int, default=15,
                        help="Headless duration in seconds (default: 15)")
    parser.add_argument("--seeds", default="42,99,7,2025,314",
                        help="Comma-separated seeds (default: 42,99,7,2025,314)")
    parser.add_argument("--history", default="logs/score_history.jsonl",
                        help="History file path")
    parser.add_argument("--skip-build", action="store_true",
                        help="Skip release build")
    args = parser.parse_args()

    seeds = [int(s.strip()) for s in args.seeds.split(",")]
    history_path = Path(args.history)
    history_path.parent.mkdir(parents=True, exist_ok=True)

    # Build
    if not args.skip_build:
        if not build_release():
            sys.exit(1)

    # Get VCS info
    vcs = get_vcs_info()
    print(f"Changeset: {vcs['change_id']} ({vcs['commit_hash']})")
    if vcs["description"] and vcs["description"] != "(no description)":
        print(f"  {vcs['description']}")
    print(f"Running {len(seeds)} seeds × {args.duration}s...")

    # Run seeds
    seed_results = {}
    constants_hash = None
    for seed in seeds:
        print(f"  seed {seed}...", end=" ", flush=True)
        result = run_seed(seed, args.duration)
        if result:
            seed_results[str(seed)] = result["seed_data"]
            if result["constants_hash"]:
                constants_hash = result["constants_hash"]
            sc = result["seed_data"]
            print(f"welfare={sc['welfare']:.3f}  agg={sc['aggregate']:.0f}  "
                  f"feat={sc['features_active']}/{sc['features_total']}  "
                  f"deaths={sc['deaths_starvation']}s/{sc['deaths_old_age']}o/{sc['deaths_injury']}i  "
                  f"cats={sc['living_cats']}")

    if not seed_results:
        print("No successful runs. Aborting.", file=sys.stderr)
        sys.exit(1)

    # Compute summary across seeds
    welfares = [s["welfare"] for s in seed_results.values()]
    aggregates = [s["aggregate"] for s in seed_results.values()]
    features = [s["features_active"] for s in seed_results.values()]
    starvations = [s["deaths_starvation"] for s in seed_results.values()]

    summary = {
        "mean_welfare": round(sum(welfares) / len(welfares), 4),
        "mean_aggregate": round(sum(aggregates) / len(aggregates), 2),
        "mean_features_active": round(sum(features) / len(features), 1),
        "total_deaths_starvation": sum(starvations),
        "seeds_with_all_dead": sum(1 for s in seed_results.values() if s["living_cats"] == 0),
        "seeds_run": len(seed_results),
        "seeds_requested": len(seeds),
    }

    # Build row
    row = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "change_id": vcs["change_id"],
        "commit_hash": vcs["commit_hash"],
        "description": vcs["description"],
        "constants_hash": constants_hash,
        "duration_secs": args.duration,
        "seeds": seed_results,
        "summary": summary,
    }

    # Append to history
    with open(history_path, "a") as f:
        f.write(json.dumps(row) + "\n")

    # Print summary
    print()
    print(f"{'='*60}")
    print(f"  changeset:  {vcs['change_id']} ({vcs['commit_hash']})")
    print(f"  constants:  {constants_hash or '?'}")
    print(f"  welfare:    {summary['mean_welfare']:.3f} (mean)")
    print(f"  aggregate:  {summary['mean_aggregate']:.0f} (mean)")
    print(f"  features:   {summary['mean_features_active']:.0f}/{seed_results[str(seeds[0])].get('features_total', '?')}")
    print(f"  starvation: {summary['total_deaths_starvation']} total across {len(seed_results)} seeds")
    print(f"  all dead:   {summary['seeds_with_all_dead']}/{len(seed_results)} seeds")
    print(f"{'='*60}")
    print(f"Appended to {history_path}")


if __name__ == "__main__":
    main()
