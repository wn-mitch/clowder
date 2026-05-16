#!/usr/bin/env python3
"""Ready-queue rollup grouped by orchestration track.

Reads docs/open-work/tickets/*.md frontmatter, filters to status=ready,
groups by orchestration track. Within coherent-block, sub-groups by block.

Default output: per-track ready count + a sample of ticket ids/titles.
--json output: machine-readable for the /work skill.

Usage:
    open_work_by_track.py            # human rollup
    open_work_by_track.py --json     # machine-readable (consumed by /work)
    open_work_by_track.py --sample 8 # show first N per group (default 5)
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--sample", type=int, default=5,
                        help="number of tickets to list per group in text mode (default 5)")
    args = parser.parse_args()

    if not TICKETS_DIR.is_dir():
        print(f"ERROR: {TICKETS_DIR} not found", file=sys.stderr)
        return 2

    by_track: dict[str, list[dict[str, str]]] = defaultdict(list)
    coherent_by_block: dict[str, list[dict[str, str]]] = defaultdict(list)

    for path in sorted(TICKETS_DIR.glob("*.md")):
        if path.name.startswith("_"):
            continue
        fm = parse_frontmatter(path)
        if fm.get("status", "").strip() != "ready":
            continue
        track = fm.get("orchestration", "").strip() or "substrate-sensitive"
        ticket = {
            "id": fm.get("id", "").strip() or path.stem.split("-", 1)[0],
            "title": fm.get("title", "").strip(),
            "cluster": fm.get("cluster", "").strip() or "—",
        }
        if track == "coherent-block":
            block = fm.get("block", "").strip() or "(unassigned)"
            ticket["block"] = block
            coherent_by_block[block].append(ticket)
        by_track[track].append(ticket)

    data = {
        "swarm-safe": by_track.get("swarm-safe", []),
        "substrate-sensitive": by_track.get("substrate-sensitive", []),
        "coherent-block": dict(coherent_by_block),
    }

    if args.json:
        print(json.dumps(data, indent=2))
        return 0

    total = sum(len(v) for v in by_track.values())
    print(f"=== ready queue by track ({total} ready tickets) ===\n")

    for track in ("swarm-safe", "substrate-sensitive"):
        rows = by_track.get(track, [])
        print(f"{track}: {len(rows)} ready")
        for t in rows[: args.sample]:
            title = t["title"][:64]
            print(f"  {t['id']:>4}  [{t['cluster']:<24}] {title}")
        if len(rows) > args.sample:
            print(f"  … ({len(rows) - args.sample} more)")
        print()

    if coherent_by_block:
        block_total = sum(len(v) for v in coherent_by_block.values())
        print(f"coherent-block: {block_total} ready across {len(coherent_by_block)} blocks")
        for block, rows in sorted(coherent_by_block.items()):
            print(f"  {block} ({len(rows)} ready)")
            for t in rows[: args.sample]:
                title = t["title"][:60]
                print(f"    {t['id']:>4}  [{t['cluster']:<24}] {title}")
            if len(rows) > args.sample:
                print(f"    … ({len(rows) - args.sample} more)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
