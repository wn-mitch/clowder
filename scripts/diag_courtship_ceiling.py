#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Ticket 148 — courtship-fondness ceiling diagnostic.

Walks every `events.jsonl` in a sweep directory and, per (seed, rep) run, emits:
  - founder_adult_count: cats whose first CatSnapshot stage is Adult or Elder
  - adult_pairs_observed: distinct ordered pairs that were concurrently Adult/Elder
    AND orientation-compatible at any fondness reading
  - pairs_crossing_friends: pairs that ever crossed friends_fondness_threshold (0.30)
    at a moment when both partners were eligible
  - max_eligible_pair_fondness: max fondness across all eligible pairs
  - courtship_tally: footer's continuity_tallies.courtship

Three hypotheses from ticket 148:
  (a) throughput — max_eligible_pair_fondness clusters at ~0.30 across seeds
      regardless of founder_adult_count
  (b) co-location — adult_pairs_observed is high but pairs_crossing_friends is low
  (c) founder-roll variance — courtship_tally bimodal by founder_adult_count

Usage:
    python3 scripts/diag_courtship_ceiling.py logs/sweep-courtship-fondness-diag
    python3 scripts/diag_courtship_ceiling.py logs/sweep-courtship-fondness-diag --json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Iterable


ADULT_STAGES = {"Adult", "Elder"}
FRIENDS_THRESHOLD = 0.30


def orient_compat(a_sex: str, a_orient: str, b_sex: str, b_orient: str) -> bool:
    """Mirror src/systems/social.rs:are_orientation_compatible."""
    if a_orient == "Asexual" or b_orient == "Asexual":
        return False

    def attracted(self_sex: str, self_orient: str, other_sex: str) -> bool:
        if self_orient == "Straight":
            return (
                self_sex != other_sex
                or other_sex == "Nonbinary"
                or self_sex == "Nonbinary"
            )
        if self_orient == "Gay":
            return (
                self_sex == other_sex
                or other_sex == "Nonbinary"
                or self_sex == "Nonbinary"
            )
        if self_orient == "Bisexual":
            return True
        return False

    return attracted(a_sex, a_orient, b_sex) and attracted(b_sex, b_orient, a_sex)


def analyze_run(events_path: Path) -> dict:
    """Single-pass scan over a run's events.jsonl."""
    cat_profile: dict[str, tuple[str, str, str]] = {}
    cat_first_stage: dict[str, str] = {}
    pair_max_fondness: dict[tuple[str, str], float] = {}
    pair_observed_eligible: set[tuple[str, str]] = set()

    courtship_tally = 0
    drift_events = 0

    with events_path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue

            if ev.get("_footer"):
                tallies = ev.get("continuity_tallies") or ev.get("_footer", {}).get(
                    "continuity_tallies", {}
                )
                courtship_tally = tallies.get("courtship", 0)
                continue

            etype = ev.get("type")
            if etype == "CourtshipDrifted":
                drift_events += 1
                continue
            if etype != "CatSnapshot":
                continue

            cat = ev.get("cat")
            if not cat:
                continue

            stage = ev.get("life_stage", "")
            sex = ev.get("sex", "")
            orient = ev.get("orientation", "")
            cat_profile[cat] = (stage, sex, orient)
            cat_first_stage.setdefault(cat, stage)

            for rel in ev.get("relationships", []):
                other = rel.get("cat")
                if not other:
                    continue
                fondness = float(rel.get("fondness", 0.0))

                other_profile = cat_profile.get(other)
                if other_profile is None:
                    continue

                other_stage, other_sex, other_orient = other_profile
                if stage not in ADULT_STAGES or other_stage not in ADULT_STAGES:
                    continue
                if not orient_compat(sex, orient, other_sex, other_orient):
                    continue

                pair = tuple(sorted([cat, other]))
                pair_observed_eligible.add(pair)
                prev = pair_max_fondness.get(pair, float("-inf"))
                if fondness > prev:
                    pair_max_fondness[pair] = fondness

    founder_adult_count = sum(
        1 for stage in cat_first_stage.values() if stage in ADULT_STAGES
    )

    pairs_crossing_friends = sum(
        1 for f in pair_max_fondness.values() if f > FRIENDS_THRESHOLD
    )

    if pair_max_fondness:
        max_pair, max_value = max(pair_max_fondness.items(), key=lambda kv: kv[1])
        max_pair_label = f"{max_pair[0]}+{max_pair[1]}"
    else:
        max_value = float("nan")
        max_pair_label = "<none>"

    return {
        "run": events_path.parent.name,
        "founder_adult_count": founder_adult_count,
        "adult_pairs_observed": len(pair_observed_eligible),
        "pairs_crossing_friends": pairs_crossing_friends,
        "max_eligible_pair_fondness": max_value,
        "max_pair": max_pair_label,
        "courtship_tally": courtship_tally,
        "drift_events_in_stream": drift_events,
    }


def find_runs(sweep_dir: Path) -> Iterable[Path]:
    for child in sorted(sweep_dir.iterdir()):
        if not child.is_dir():
            continue
        events = child / "events.jsonl"
        if events.exists() and events.stat().st_size > 0:
            yield events


def parse_seed_rep(name: str) -> tuple[int, int]:
    parts = name.rsplit("-", 1)
    if len(parts) != 2:
        return (0, 0)
    try:
        return (int(parts[0]), int(parts[1]))
    except ValueError:
        return (0, 0)


def render_text(rows: list[dict]) -> str:
    headers = [
        "run",
        "founder_adults",
        "elig_pairs",
        "pairs>0.30",
        "max_fondness",
        "max_pair",
        "courtship_tally",
    ]
    lines = ["\t".join(headers)]
    for row in rows:
        lines.append(
            "\t".join(
                [
                    row["run"],
                    str(row["founder_adult_count"]),
                    str(row["adult_pairs_observed"]),
                    str(row["pairs_crossing_friends"]),
                    f"{row['max_eligible_pair_fondness']:.4f}",
                    row["max_pair"],
                    str(row["courtship_tally"]),
                ]
            )
        )

    by_seed: dict[int, list[dict]] = {}
    for row in rows:
        seed, _ = parse_seed_rep(row["run"])
        by_seed.setdefault(seed, []).append(row)

    lines.append("")
    lines.append("== per-seed summary ==")
    lines.append(
        "seed\tn_reps\tmean_max_fondness\tmax_max_fondness\tmean_courtship_tally\tmean_founder_adults"
    )
    for seed, group in sorted(by_seed.items()):
        n = len(group)
        max_fondnesses = [
            r["max_eligible_pair_fondness"]
            for r in group
            if r["max_eligible_pair_fondness"] == r["max_eligible_pair_fondness"]
        ]
        if max_fondnesses:
            mean_mf = sum(max_fondnesses) / len(max_fondnesses)
            max_mf = max(max_fondnesses)
        else:
            mean_mf = float("nan")
            max_mf = float("nan")
        mean_ct = sum(r["courtship_tally"] for r in group) / n
        mean_adults = sum(r["founder_adult_count"] for r in group) / n
        lines.append(
            f"{seed}\t{n}\t{mean_mf:.4f}\t{max_mf:.4f}\t{mean_ct:.1f}\t{mean_adults:.2f}"
        )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sweep_dir", type=Path, help="Path to logs/sweep-<label>/")
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit per-run JSON list on stdout instead of the text summary.",
    )
    args = parser.parse_args()

    if not args.sweep_dir.is_dir():
        print(f"Not a directory: {args.sweep_dir}", file=sys.stderr)
        return 2

    rows = [analyze_run(p) for p in find_runs(args.sweep_dir)]
    if not rows:
        print(f"No runs with events.jsonl found under {args.sweep_dir}", file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(rows, indent=2))
    else:
        print(render_text(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main())
