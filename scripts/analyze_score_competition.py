#!/usr/bin/env python3
"""Analyze per-action scoring distributions from a soak's CatSnapshot events.

Given an events.jsonl produced by a run carrying the full-score-distribution
instrumentation (commit 290a5d9d or later — prior runs only stored top-3),
computes:

  * gate_open_fraction: fraction of CatSnapshots where the action was scored
    at all (i.e., its context gate was open)
  * top1_fraction: fraction of CatSnapshots where the action won
  * mean_score: mean score when the action was scored
  * rank_distribution: tallied rank positions (1 = winner)

And for pair diagnostics:

  * co_occurrence(A, B): fraction of snapshots where both A and B were scored
  * margin(A, B): mean (score_A - score_B) on co-occurring snapshots
  * a_wins_vs_b: fraction of co-occurring snapshots where A out-scored B

Usage:
  scripts/analyze_score_competition.py logs/tuned-42/events.jsonl
  scripts/analyze_score_competition.py --pair Mate Socialize logs/tuned-42/events.jsonl
  scripts/analyze_score_competition.py --compare logs/baseline/events.jsonl logs/treatment/events.jsonl
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path


def iter_cat_snapshots(path: Path):
    with path.open() as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            if obj.get("type") == "CatSnapshot":
                yield obj


def analyze(path: Path) -> dict:
    total = 0
    gate_open = Counter()
    top1 = Counter()
    rank_tally: dict[str, Counter[int]] = defaultdict(Counter)
    score_values: dict[str, list[float]] = defaultdict(list)
    co_occurrence: dict[tuple[str, str], int] = Counter()
    pair_margins: dict[tuple[str, str], list[float]] = defaultdict(list)
    winner: Counter = Counter()  # actual CurrentAction.action wins

    for snap in iter_cat_snapshots(path):
        total += 1
        scores = snap.get("last_scores") or []
        if not scores:
            continue
        winner[snap.get("current_action", "?")] += 1

        present_actions = []
        for rank, entry in enumerate(scores):
            action, score = entry[0], entry[1]
            present_actions.append((action, score))
            gate_open[action] += 1
            rank_tally[action][rank + 1] += 1
            score_values[action].append(score)
            if rank == 0:
                top1[action] += 1

        for i in range(len(present_actions)):
            for j in range(i + 1, len(present_actions)):
                a, sa = present_actions[i]
                b, sb = present_actions[j]
                key = tuple(sorted((a, b)))
                co_occurrence[key] += 1
                margin = sa - sb if key == (a, b) else sb - sa
                # margin stored as (first_in_key - second_in_key)
                pair_margins[key].append(margin)

    per_action = {}
    for action in sorted(gate_open.keys()):
        scores = score_values[action]
        per_action[action] = {
            "gate_open_count": gate_open[action],
            "gate_open_fraction": gate_open[action] / total if total else 0.0,
            "top1_count": top1[action],
            "top1_fraction": top1[action] / total if total else 0.0,
            "mean_score": statistics.fmean(scores) if scores else 0.0,
            "median_score": statistics.median(scores) if scores else 0.0,
            "rank_distribution": dict(rank_tally[action].most_common()),
        }

    return {
        "path": str(path),
        "total_snapshots": total,
        "per_action": per_action,
        "co_occurrence": {f"{a}|{b}": cnt for (a, b), cnt in co_occurrence.items()},
        "pair_margins_mean": {
            f"{a}|{b}": statistics.fmean(margins)
            for (a, b), margins in pair_margins.items()
            if margins
        },
        "winner_counter": dict(winner),
    }


def format_table(result: dict) -> str:
    lines = []
    lines.append(f"=== {result['path']} ===")
    lines.append(f"total CatSnapshot records: {result['total_snapshots']}")
    lines.append("")
    lines.append(f"{'Action':<16} {'gate%':>7} {'top1%':>7} {'meanSc':>8} {'medSc':>8}")
    pa = result["per_action"]
    for action in sorted(pa, key=lambda a: -pa[a]["gate_open_fraction"]):
        d = pa[action]
        lines.append(
            f"{action:<16} {d['gate_open_fraction'] * 100:>6.1f}%"
            f" {d['top1_fraction'] * 100:>6.1f}%"
            f" {d['mean_score']:>8.3f}"
            f" {d['median_score']:>8.3f}"
        )
    return "\n".join(lines)


def pair_report(result: dict, a: str, b: str) -> str:
    key = "|".join(sorted((a, b)))
    co = result["co_occurrence"].get(key, 0)
    margin_mean = result["pair_margins_mean"].get(key, None)
    pa = result["per_action"]
    total = result["total_snapshots"]
    a_gate = pa.get(a, {}).get("gate_open_count", 0)
    b_gate = pa.get(b, {}).get("gate_open_count", 0)

    # Determine which is "first_in_key"
    sorted_pair = tuple(sorted((a, b)))
    margin_direction = (
        f"{sorted_pair[0]} - {sorted_pair[1]}" if margin_mean is not None else "n/a"
    )

    lines = [
        f"--- Pair: {a} vs {b} ---",
        f"  {a} gate-open:       {a_gate} / {total} ({a_gate / total * 100:.1f}%)",
        f"  {b} gate-open:       {b_gate} / {total} ({b_gate / total * 100:.1f}%)",
        f"  co-occurrence:       {co} / {total} ({co / total * 100:.1f}%)",
        f"  mean margin ({margin_direction}): {margin_mean:+.3f}"
        if margin_mean is not None
        else "  mean margin: n/a (pair never co-occurred)",
    ]
    return "\n".join(lines)


def compare(r1: dict, r2: dict) -> str:
    lines = []
    lines.append(f"Δ (treatment - baseline)  [{r2['path']} vs {r1['path']}]")
    lines.append("")
    lines.append(
        f"{'Action':<16} "
        f"{'gate% base':>10} {'gate% treat':>11} {'Δgate':>7} "
        f"{'top1% base':>10} {'top1% treat':>11} {'Δtop1':>7}"
    )
    actions = sorted(set(r1["per_action"]) | set(r2["per_action"]))
    for action in actions:
        d1 = r1["per_action"].get(action, {})
        d2 = r2["per_action"].get(action, {})
        g1 = d1.get("gate_open_fraction", 0.0) * 100
        g2 = d2.get("gate_open_fraction", 0.0) * 100
        t1 = d1.get("top1_fraction", 0.0) * 100
        t2 = d2.get("top1_fraction", 0.0) * 100
        lines.append(
            f"{action:<16} "
            f"{g1:>9.1f}% {g2:>10.1f}% {g2 - g1:>+6.1f} "
            f"{t1:>9.1f}% {t2:>10.1f}% {t2 - t1:>+6.1f}"
        )
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("logs", nargs="+", type=Path)
    ap.add_argument(
        "--pair",
        nargs=2,
        metavar=("A", "B"),
        help="Print pair diagnostics (co-occurrence, margin) for actions A and B.",
    )
    ap.add_argument(
        "--compare",
        action="store_true",
        help="Compare two logs side-by-side (provide baseline then treatment).",
    )
    ap.add_argument("--json", action="store_true", help="Emit raw JSON instead of tables.")
    args = ap.parse_args()

    results = [analyze(path) for path in args.logs]

    if args.json:
        json.dump(results, sys.stdout, indent=2, default=str)
        print()
        return

    for r in results:
        print(format_table(r))
        if args.pair:
            print()
            print(pair_report(r, *args.pair))
        print()

    if args.compare and len(results) == 2:
        print(compare(results[0], results[1]))


if __name__ == "__main__":
    main()
