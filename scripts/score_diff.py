#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# ///
"""
Score history diff viewer for Clowder simulation.

Reads logs/score_history.jsonl and shows comparisons between benchmark runs.

Usage:
    uv run scripts/score_diff.py              # compare last two entries
    uv run scripts/score_diff.py --last 5     # show last 5 entries as table
    uv run scripts/score_diff.py --all        # show full history
"""

import argparse
import json
import sys
from pathlib import Path


def load_history(path: str) -> list[dict]:
    entries = []
    p = Path(path)
    if not p.exists():
        return entries
    with open(p) as f:
        for line in f:
            line = line.strip()
            if line:
                entries.append(json.loads(line))
    return entries


def fmt_delta(old: float, new: float, is_int: bool = False) -> str:
    """Format a delta with direction indicator."""
    if old == 0 and new == 0:
        return "(=)"
    if old == 0:
        return f"(+{new:.0f})" if is_int else f"(+{new})"
    delta = new - old
    pct = (delta / abs(old)) * 100 if old != 0 else 0
    if is_int:
        if delta == 0:
            return "(=)"
        sign = "+" if delta > 0 else ""
        return f"({sign}{delta:.0f})"
    if abs(delta) < 0.0001:
        return "(=)"
    sign = "+" if delta > 0 else ""
    return f"({sign}{pct:.1f}%)"


def print_comparison(old: dict, new: dict):
    os = old["summary"]
    ns = new["summary"]
    oc = old.get("constants_hash", "?")
    nc = new.get("constants_hash", "?")

    ts_old = old.get("timestamp", "?")[:19]
    ts_new = new.get("timestamp", "?")[:19]

    print(f"Score comparison: {old['change_id']} -> {new['change_id']}")
    print(f"  timestamps: {ts_old} -> {ts_new}")
    print(f"  duration:   {old.get('duration_secs', '?')}s -> {new.get('duration_secs', '?')}s")
    print()

    # Schema v2 renamed mean_features_active → mean_positive_features_active
    # and added mean_negative_events. Old entries fall back gracefully.
    old_pos = os.get("mean_positive_features_active", os.get("mean_features_active", 0))
    new_pos = ns.get("mean_positive_features_active", ns.get("mean_features_active", 0))
    old_neg = os.get("mean_negative_events", 0)
    new_neg = ns.get("mean_negative_events", 0)

    rows = [
        ("welfare", os["mean_welfare"], ns["mean_welfare"], False),
        ("aggregate", os["mean_aggregate"], ns["mean_aggregate"], False),
        ("feat+", old_pos, new_pos, True),
        ("neg events", old_neg, new_neg, True),
        ("starvation", os["total_deaths_starvation"], ns["total_deaths_starvation"], True),
        ("all dead", os["seeds_with_all_dead"], ns["seeds_with_all_dead"], True),
    ]

    old_schema = os.get("score_schema_version", 1)
    new_schema = ns.get("score_schema_version", 1)
    if old_schema != new_schema:
        print(f"  ⚠ schema change: v{old_schema} -> v{new_schema} — aggregate shift may be definitional, not behavioral")
        print()

    for label, o, n, is_int in rows:
        if is_int:
            fmt_o = f"{o:.0f}"
            fmt_n = f"{n:.0f}"
        else:
            fmt_o = f"{o:.4f}"
            fmt_n = f"{n:.4f}"
        delta = fmt_delta(o, n, is_int)
        print(f"  {label:12s}  {fmt_o:>10s} -> {fmt_n:<10s}  {delta}")

    constants_changed = oc != nc
    print(f"  {'constants':12s}  {(oc or '?'):>10s} -> {(nc or '?'):<10s}  {'(CHANGED)' if constants_changed else '(=)'}")

    # Per-seed breakdown
    old_seeds = old.get("seeds", {})
    new_seeds = new.get("seeds", {})
    all_seed_keys = sorted(set(old_seeds) | set(new_seeds), key=lambda x: int(x))

    if all_seed_keys:
        print()
        print(f"  {'seed':>6s}  {'welfare':>8s}  {'agg':>7s}  {'feat+':>5s}  {'starv':>5s}  {'cats':>4s}")
        print(f"  {'-'*6}  {'-'*8}  {'-'*7}  {'-'*5}  {'-'*5}  {'-'*4}")
        for sk in all_seed_keys:
            ns_seed = new_seeds.get(sk, {})
            os_seed = old_seeds.get(sk, {})
            w_new = ns_seed.get("welfare", 0)
            w_old = os_seed.get("welfare", 0)
            a_new = ns_seed.get("aggregate", 0)
            f_new = ns_seed.get("positive_features_active", ns_seed.get("features_active", 0))
            s_new = ns_seed.get("deaths_starvation", 0)
            c_new = ns_seed.get("living_cats", 0)
            delta_w = fmt_delta(w_old, w_new)
            print(f"  {sk:>6s}  {w_new:>8.3f}  {a_new:>7.0f}  {f_new:>5d}  {s_new:>5d}  {c_new:>4d}  {delta_w}")


def print_table(entries: list[dict]):
    if not entries:
        print("No entries in history.")
        return

    print(f"{'#':>3s}  {'change':>12s}  {'timestamp':>19s}  {'welfare':>8s}  {'agg':>7s}  {'feat+':>5s}  {'starv':>5s}  {'dead':>4s}  {'hash':>16s}  description")
    print(f"{'─'*3}  {'─'*12}  {'─'*19}  {'─'*8}  {'─'*7}  {'─'*5}  {'─'*5}  {'─'*4}  {'─'*16}  {'─'*30}")

    for i, entry in enumerate(entries):
        s = entry.get("summary", {})
        ts = entry.get("timestamp", "?")[:19]
        desc = entry.get("description", "")[:30]
        ch = entry.get("constants_hash", "?") or "?"
        # Schema v2: mean_positive_features_active; v1: mean_features_active
        feats = s.get("mean_positive_features_active", s.get("mean_features_active", 0))
        print(
            f"{i+1:>3d}  {entry.get('change_id', '?'):>12s}  {ts:>19s}  "
            f"{s.get('mean_welfare', 0):>8.4f}  {s.get('mean_aggregate', 0):>7.0f}  "
            f"{feats:>5.0f}  {s.get('total_deaths_starvation', 0):>5d}  "
            f"{s.get('seeds_with_all_dead', 0):>4d}  {ch:>16s}  {desc}"
        )


def main():
    parser = argparse.ArgumentParser(description="Clowder score history viewer")
    parser.add_argument("--history", default="logs/score_history.jsonl",
                        help="History file path")
    parser.add_argument("--all", action="store_true",
                        help="Show full history as table")
    parser.add_argument("--last", type=int, default=0,
                        help="Show last N entries as table")
    args = parser.parse_args()

    entries = load_history(args.history)
    if not entries:
        print(f"No history found at {args.history}. Run `just score-track` first.")
        sys.exit(1)

    if args.all:
        print_table(entries)
    elif args.last > 0:
        print_table(entries[-args.last:])
    elif len(entries) < 2:
        print("Only 1 entry — showing as table (need 2+ for diff):")
        print_table(entries)
    else:
        print_comparison(entries[-2], entries[-1])


if __name__ == "__main__":
    main()
