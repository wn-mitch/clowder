#!/usr/bin/env python3
"""Coherent-block introspection primitive.

Stage 1.5 of ticket 354. Composes against the orchestration: frontmatter
axis defined in Stage 1.1 + the Ticket dataclass extensions in
scripts/_ticket_frontmatter.py.

Modes:
    block_info.py list                   # rollup of all blocks
    block_info.py <initiative-id>        # detail view of one block
    block_info.py list --json            # machine-readable rollup
    block_info.py <id> --json            # machine-readable detail

A "block" is identified by the `block:` frontmatter field; every
coherent-block ticket carries one. The block-anchor (≤1 per block) is
the ticket whose landing fires the block-level verdict.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from _ticket_frontmatter import Ticket, load_tickets  # noqa: E402

TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"


def gather_blocks(tickets: list[Ticket]) -> dict[str, dict]:
    blocks: dict[str, dict] = defaultdict(
        lambda: {"tickets": [], "anchor": None, "statuses": defaultdict(int)}
    )
    for t in tickets:
        if t.orchestration != "coherent-block":
            continue
        if not t.block:
            continue
        blocks[t.block]["tickets"].append(t)
        blocks[t.block]["statuses"][t.status] += 1
        if t.verdict_anchor:
            blocks[t.block]["anchor"] = t
    return blocks


def cmd_list(emit_json: bool) -> int:
    tickets = load_tickets(TICKETS_DIR)
    blocks = gather_blocks(tickets)

    if emit_json:
        out = []
        for name in sorted(blocks):
            b = blocks[name]
            out.append({
                "block": name,
                "ticket_count": len(b["tickets"]),
                "anchor": b["anchor"].id if b["anchor"] else None,
                "statuses": dict(b["statuses"]),
            })
        print(json.dumps(out, indent=2))
        return 0

    if not blocks:
        print("blocks: none (no coherent-block tickets in corpus)")
        return 0

    print(f"{'BLOCK':<30} {'TICKETS':<8} {'ANCHOR':<8} STATUSES")
    for name in sorted(blocks):
        b = blocks[name]
        anchor = b["anchor"].id if b["anchor"] else "(none)"
        statuses = " ".join(f"{s}:{n}" for s, n in sorted(b["statuses"].items()))
        print(f"{name:<30} {len(b['tickets']):<8} {anchor:<8} {statuses}")
    return 0


def cmd_detail(block_name: str, emit_json: bool) -> int:
    tickets = load_tickets(TICKETS_DIR)
    blocks = gather_blocks(tickets)

    if block_name not in blocks:
        print(f"ERROR: no block named '{block_name}' "
              f"(available: {sorted(blocks)})", file=sys.stderr)
        return 1

    b = blocks[block_name]
    block_tickets = sorted(b["tickets"], key=lambda t: t.id)

    if emit_json:
        out = {
            "block": block_name,
            "anchor": b["anchor"].id if b["anchor"] else None,
            "tickets": [
                {
                    "id": t.id,
                    "title": t.title,
                    "status": t.status,
                    "cluster": t.cluster,
                    "is_anchor": t.verdict_anchor,
                    "blocked_by": t.blocked_by,
                    "path": str(t.path.relative_to(REPO_ROOT)),
                }
                for t in block_tickets
            ],
            "statuses": dict(b["statuses"]),
        }
        print(json.dumps(out, indent=2))
        return 0

    anchor = b["anchor"].id if b["anchor"] else "(none)"
    print(f"=== block: {block_name} ===")
    print(f"  tickets:   {len(block_tickets)}")
    print(f"  anchor:    {anchor}")
    print(f"  statuses:  {' '.join(f'{s}:{n}' for s, n in sorted(b['statuses'].items()))}")
    print()
    print(f"  {'ID':<6} {'STATUS':<14} {'ANCHOR':<8} TITLE")
    for t in block_tickets:
        is_anchor = "★" if t.verdict_anchor else " "
        print(f"  {t.id:<6} {t.status:<14} {is_anchor:<8} {t.title}")

    if not b["anchor"]:
        print()
        print("WARN: this block has no verdict-anchor yet.")
        print("      Mark one with: just retag <id> --anchor")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("target", help="'list' or a block name")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    if args.target == "list":
        return cmd_list(args.json)
    return cmd_detail(args.target, args.json)


if __name__ == "__main__":
    sys.exit(main())
