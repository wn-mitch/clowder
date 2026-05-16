#!/usr/bin/env python3
"""Set or clear the verdict-anchor on a coherent-block ticket.

A block-anchor is the single ticket whose landing fires the block-level
verdict (asserting the orthogonality precondition for the block's legs).
Invariant: ≤1 verdict-anchor: true per block — also enforced by
scripts/check_orchestration_frontmatter.py, but block-anchor refuses up-front
with a clearer error.

Composes scripts/retag.sh (which already supports --anchor / --unset-anchor)
and regenerates docs/open-work.md after the edit.

Usage:
    block-anchor.py <block-id> <ticket-id>           # set anchor
    block-anchor.py <block-id> <ticket-id> --clear   # remove anchor
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from _ticket_frontmatter import load_tickets  # noqa: E402

TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("block", help="block name (matches `block:` frontmatter field)")
    parser.add_argument("ticket_id", help="ticket id to set/clear as anchor")
    parser.add_argument("--clear", action="store_true",
                        help="remove the verdict-anchor instead of setting it")
    args = parser.parse_args()

    tickets = load_tickets(TICKETS_DIR)
    block_tickets = [t for t in tickets if t.block == args.block]
    if not block_tickets:
        print(f"ERROR: no tickets found with block '{args.block}'", file=sys.stderr)
        return 2

    target = next((t for t in block_tickets if t.id == args.ticket_id), None)
    if target is None:
        members = ", ".join(t.id for t in block_tickets)
        print(
            f"ERROR: ticket {args.ticket_id} is not in block '{args.block}'.\n"
            f"  Block members: {members}",
            file=sys.stderr,
        )
        return 2

    existing_anchor = next((t for t in block_tickets if t.verdict_anchor), None)

    if args.clear:
        if not target.verdict_anchor:
            print(f"block-anchor: ticket {target.id} is not the anchor of '{args.block}' — no-op")
            return 0
        retag_args = ["--unset-anchor"]
        action = "cleared"
    else:
        if existing_anchor and existing_anchor.id != target.id:
            print(
                f"ERROR: block '{args.block}' already has anchor {existing_anchor.id} "
                f"({existing_anchor.path.name}).\n"
                f"  Clear it first: just block-anchor {args.block} {existing_anchor.id} --clear",
                file=sys.stderr,
            )
            return 1
        if target.verdict_anchor:
            print(f"block-anchor: ticket {target.id} is already the anchor of '{args.block}' — no-op")
            return 0
        retag_args = ["--anchor"]
        action = "set"

    cmd = ["bash", str(REPO_ROOT / "scripts" / "retag.sh"), target.id, *retag_args]
    result = subprocess.run(cmd, cwd=REPO_ROOT, check=False)
    if result.returncode != 0:
        print(f"ERROR: retag.sh failed (exit {result.returncode})", file=sys.stderr)
        return result.returncode

    # Regenerate the open-work index — matches the post-edit pattern in retag.sh.
    subprocess.run(
        ["just", "open-work-index"], cwd=REPO_ROOT, check=False,
        capture_output=True,
    )
    print(f"block-anchor: {action} on {target.id} ({target.path.name})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
