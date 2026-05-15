#!/usr/bin/env python3
"""Audit corpus orchestration tagging — rollup view + invariant violations.

Reports per-track / per-cluster / per-initiative / per-block counts,
plus any invariant violations (defers to
check_orchestration_frontmatter.py logic).

Usage:
    retag_audit.py              # human-readable rollup
    retag_audit.py --json       # machine-readable (used by /retag skill)
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

TICKETS_DIR = Path("docs/open-work/tickets")
VALID_TRACKS = ("substrate-sensitive", "coherent-block", "swarm-safe")


def parse_frontmatter(path: Path) -> dict[str, str]:
    fm: dict[str, str] = {}
    in_fm = False
    seen_open = False
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line.strip() == "---":
                if not seen_open:
                    seen_open = True
                    in_fm = True
                    continue
                break
            if not in_fm:
                continue
            m = re.match(r"^([A-Za-z][\w-]*):\s*(.*)$", line)
            if m:
                fm[m.group(1)] = m.group(2).strip()
    return fm


def parse_list(raw: str) -> list[str]:
    raw = raw.strip()
    if not raw or raw == "[]":
        return []
    if raw.startswith("[") and raw.endswith("]"):
        inner = raw[1:-1].strip()
        if not inner:
            return []
        return [s.strip().strip('"').strip("'") for s in inner.split(",")]
    return [raw.strip().strip('"').strip("'")]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    if not TICKETS_DIR.is_dir():
        print(f"ERROR: {TICKETS_DIR} not found", file=sys.stderr)
        return 2

    by_track: Counter[str] = Counter()
    by_track_status: dict[str, Counter[str]] = defaultdict(Counter)
    by_track_cluster: dict[str, Counter[str]] = defaultdict(Counter)
    blocks: dict[str, dict[str, object]] = {}
    untagged: list[str] = []
    seen = 0

    for path in sorted(TICKETS_DIR.glob("*.md")):
        if path.name.startswith("_"):
            continue
        seen += 1
        fm = parse_frontmatter(path)
        track = fm.get("orchestration", "").strip()
        cluster = fm.get("cluster", "").strip() or "—"
        status = fm.get("status", "").strip() or "—"
        block = fm.get("block", "").strip()
        anchor = fm.get("verdict-anchor", "").strip() == "true"
        ticket_id = fm.get("id", "").strip() or path.stem

        if not track:
            untagged.append(path.name)
            continue

        by_track[track] += 1
        by_track_status[track][status] += 1
        by_track_cluster[track][cluster] += 1

        if track == "coherent-block" and block:
            b = blocks.setdefault(block, {"tickets": [], "anchor": None})
            b["tickets"].append(ticket_id)
            if anchor:
                b["anchor"] = ticket_id

    data = {
        "seen": seen,
        "untagged": untagged,
        "by_track": dict(by_track),
        "by_track_status": {t: dict(c) for t, c in by_track_status.items()},
        "by_track_cluster": {t: dict(c) for t, c in by_track_cluster.items()},
        "blocks": blocks,
    }

    if args.json:
        print(json.dumps(data, indent=2))
        return 0

    print(f"=== orchestration audit ({seen} active tickets) ===\n")
    print("Track totals:")
    for t in VALID_TRACKS:
        print(f"  {t:<22} {by_track.get(t, 0):>4}")
    if untagged:
        print(f"\n  UNTAGGED:              {len(untagged):>4}  (run 'just retag-init')")
        for u in untagged[:5]:
            print(f"    {u}")
        if len(untagged) > 5:
            print(f"    ... ({len(untagged) - 5} more)")
    print()

    for t in VALID_TRACKS:
        if not by_track_status.get(t):
            continue
        print(f"{t} by status:")
        for s, n in sorted(by_track_status[t].items()):
            print(f"  {s:<14} {n:>4}")
        print(f"{t} by cluster:")
        for c, n in sorted(by_track_cluster[t].items()):
            print(f"  {c:<28} {n:>4}")
        print()

    if blocks:
        print(f"Blocks ({len(blocks)}):")
        for name, info in sorted(blocks.items()):
            anchor = info["anchor"] or "(none)"
            tickets = info["tickets"]
            print(f"  {name:<28} tickets={len(tickets):>3}  anchor={anchor}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
