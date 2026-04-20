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

PHASE_NAMES = ("Dawn", "Day", "Dusk", "Night")


def percentile(values: list[float], q: float) -> float:
    """Nearest-rank percentile over a sorted copy. `q` in [0, 1]."""
    if not values:
        return 0.0
    s = sorted(values)
    idx = min(len(s) - 1, int(round(q * (len(s) - 1))))
    return s[idx]


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

        # Parse events — find last ColonyScore, last SystemActivation, header,
        # and collect per-snapshot fields for percentile / phase-binned
        # distributions. The per-snapshot pass is cheap: one pointer per
        # CatSnapshot event, no deserialization of nested personality/skills.
        header = None
        last_score = None
        last_activation = None
        energies: list[float] = []
        mood_valences: list[float] = []
        # Index 0=Dawn, 1=Day, 2=Dusk, 3=Night
        sleep_by_phase = [0, 0, 0, 0]
        snapshots_by_phase = [0, 0, 0, 0]

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
                elif event_type == "CatSnapshot":
                    needs = obj.get("needs") or {}
                    if "energy" in needs:
                        energies.append(float(needs["energy"]))
                    if "mood_valence" in obj:
                        mood_valences.append(float(obj["mood_valence"]))
                    # Phase binning: (tick // ticks_per_day_phase) % 4.
                    # We may not have `header` yet on the very first snapshot
                    # line in pathological files — guard.
                    if header is not None:
                        tpp = (header.get("sim_config") or {}).get("ticks_per_day_phase")
                        tick = obj.get("tick")
                        if tpp and tick is not None:
                            phase_idx = (int(tick) // int(tpp)) % 4
                            snapshots_by_phase[phase_idx] += 1
                            if obj.get("current_action") == "Sleep":
                                sleep_by_phase[phase_idx] += 1

        if last_score is None:
            print(f"  seed {seed}: no ColonyScore event")
            return None

        # Extract constants hash from header
        constants_hash = None
        if header and "constants" in header:
            constants_json = json.dumps(header["constants"], sort_keys=True)
            constants_hash = hashlib.sha256(constants_json.encode()).hexdigest()[:16]

        # Build per-seed summary. Schema v3 adds per-snapshot percentiles
        # (energy, mood_valence) and phase-binned Sleep share — the metrics
        # the sleep-phase-bias rollout predicts. Schema v2 still readable;
        # downstream tooling should branch on score_schema_version.
        sleep_share = {}
        for i, name in enumerate(PHASE_NAMES):
            denom = snapshots_by_phase[i]
            sleep_share[name] = round(sleep_by_phase[i] / denom, 4) if denom > 0 else 0.0

        # Cook events — extracted from the last SystemActivation snapshot.
        # FoodCooked is a Positive feature that fires each time a cat flips a
        # raw item's `cooked` flag at a Kitchen.
        food_cooked_total = 0
        if last_activation:
            food_cooked_total = int(
                (last_activation.get("positive") or {}).get("FoodCooked", 0)
            )

        seed_data = {
            "score_schema_version": 4,
            "welfare": round(last_score.get("welfare", 0), 4),
            "aggregate": round(last_score.get("aggregate", 0), 2),
            "positive_activation_score": round(last_score.get("positive_activation_score", 0), 2),
            "positive_features_active": last_score.get("positive_features_active", 0),
            "positive_features_total": last_score.get("positive_features_total", 0),
            "negative_events_total": last_score.get("negative_events_total", 0),
            "neutral_features_active": last_score.get("neutral_features_active", 0),
            "neutral_features_total": last_score.get("neutral_features_total", 0),
            "deaths_starvation": last_score.get("deaths_starvation", 0),
            "deaths_old_age": last_score.get("deaths_old_age", 0),
            "deaths_injury": last_score.get("deaths_injury", 0),
            "living_cats": last_score.get("living_cats", 0),
            "seasons_survived": last_score.get("seasons_survived", 0),
            "bonds_formed": last_score.get("bonds_formed", 0),
            "aspirations_completed": last_score.get("aspirations_completed", 0),
            "kittens_born": last_score.get("kittens_born", 0),
            # Percentiles over per-snapshot cat state. p50 is the headline;
            # p10/p90 reveal whether a change lifts the floor, pulls the
            # ceiling, or both.
            "energy_p10": round(percentile(energies, 0.10), 4),
            "energy_p50": round(percentile(energies, 0.50), 4),
            "energy_p90": round(percentile(energies, 0.90), 4),
            "mood_valence_p50": round(percentile(mood_valences, 0.50), 4),
            "snapshots_total": len(energies),
            # Conditional share: of snapshots taken during {phase}, what
            # fraction had current_action == Sleep. Four floats, each in
            # [0, 1], not constrained to sum to 1 (they're independent
            # conditional probabilities across phases).
            "sleep_share_by_phase": sleep_share,
            # Count of completed Cook actions in this run. Non-zero is the
            # signal that the Kitchen pipeline is live.
            "food_cooked_total": food_cooked_total,
        }

        # Dead positive features — these are the ones that matter for the
        # "did a system go dead?" canary. A dead negative feature is fine
        # (means nothing bad happened); a dead neutral feature is a weak
        # signal. Keep per-category lists for analysis tooling.
        if last_activation:
            seed_data["dead_positive_features"] = sorted(
                k for k, v in last_activation.get("positive", {}).items() if v == 0
            )
            seed_data["dead_neutral_features"] = sorted(
                k for k, v in last_activation.get("neutral", {}).items() if v == 0
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
            sbp = sc.get("sleep_share_by_phase", {})
            print(f"welfare={sc['welfare']:.3f}  agg={sc['aggregate']:.0f}  "
                  f"feat+={sc['positive_features_active']}/{sc['positive_features_total']}  "
                  f"neg={sc['negative_events_total']}  "
                  f"neut+={sc['neutral_features_active']}/{sc['neutral_features_total']}  "
                  f"deaths={sc['deaths_starvation']}s/{sc['deaths_old_age']}o/{sc['deaths_injury']}i  "
                  f"cats={sc['living_cats']}  "
                  f"kittens={sc.get('kittens_born', 0)}  "
                  f"cooks={sc.get('food_cooked_total', 0)}  "
                  f"e_p50={sc.get('energy_p50', 0):.2f}  "
                  f"m_p50={sc.get('mood_valence_p50', 0):.2f}  "
                  f"sleepN={sbp.get('Night', 0):.2f}")

    if not seed_results:
        print("No successful runs. Aborting.", file=sys.stderr)
        sys.exit(1)

    # Compute summary across seeds
    welfares = [s["welfare"] for s in seed_results.values()]
    aggregates = [s["aggregate"] for s in seed_results.values()]
    positive_features = [s["positive_features_active"] for s in seed_results.values()]
    negative_events = [s["negative_events_total"] for s in seed_results.values()]
    starvations = [s["deaths_starvation"] for s in seed_results.values()]
    energy_p50s = [s.get("energy_p50", 0.0) for s in seed_results.values()]
    mood_p50s = [s.get("mood_valence_p50", 0.0) for s in seed_results.values()]
    kittens = [s.get("kittens_born", 0) for s in seed_results.values()]
    cooked = [s.get("food_cooked_total", 0) for s in seed_results.values()]

    def mean_phase_share(name: str) -> float:
        shares = [
            (s.get("sleep_share_by_phase") or {}).get(name, 0.0)
            for s in seed_results.values()
        ]
        return round(sum(shares) / len(shares), 4) if shares else 0.0

    summary = {
        "score_schema_version": 4,
        "mean_welfare": round(sum(welfares) / len(welfares), 4),
        "mean_aggregate": round(sum(aggregates) / len(aggregates), 2),
        "mean_positive_features_active": round(sum(positive_features) / len(positive_features), 1),
        "mean_negative_events": round(sum(negative_events) / len(negative_events), 1),
        "total_deaths_starvation": sum(starvations),
        "seeds_with_all_dead": sum(1 for s in seed_results.values() if s["living_cats"] == 0),
        "seeds_run": len(seed_results),
        "seeds_requested": len(seeds),
        "mean_energy_p50": round(sum(energy_p50s) / len(energy_p50s), 4),
        "mean_mood_valence_p50": round(sum(mood_p50s) / len(mood_p50s), 4),
        "total_kittens_born": sum(kittens),
        "total_food_cooked": sum(cooked),
        "mean_sleep_share_by_phase": {
            name: mean_phase_share(name) for name in PHASE_NAMES
        },
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
    first_seed = seed_results[str(seeds[0])]
    print(f"  positive:   {summary['mean_positive_features_active']:.0f}/{first_seed.get('positive_features_total', '?')} features firing")
    print(f"  negative:   {summary['mean_negative_events']:.1f} events (mean)")
    print(f"  starvation: {summary['total_deaths_starvation']} total across {len(seed_results)} seeds")
    print(f"  all dead:   {summary['seeds_with_all_dead']}/{len(seed_results)} seeds")
    print(f"  kittens:    {summary['total_kittens_born']} total across {len(seed_results)} seeds")
    print(f"  energy_p50: {summary['mean_energy_p50']:.3f} (mean)")
    print(f"  mood_p50:   {summary['mean_mood_valence_p50']:.3f} (mean)")
    phase_shares = summary["mean_sleep_share_by_phase"]
    print(f"  sleep-by-phase: Dawn={phase_shares['Dawn']:.2f}  "
          f"Day={phase_shares['Day']:.2f}  Dusk={phase_shares['Dusk']:.2f}  "
          f"Night={phase_shares['Night']:.2f}")
    print(f"{'='*60}")
    print(f"Appended to {history_path}")


if __name__ == "__main__":
    main()
