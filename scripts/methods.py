#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
HTN method-registry audit surface (`just methods`).

Collaborative-use tool: "what's left, dormant-method-wise?" answered in
a single tool call. Composes the bash check script's `--list-json` mode
(single parse source-of-truth, ticket 319) with a small formatter.

Usage:
    just methods                # list all registered methods
    just methods --pending      # only PendingSubstrate (dormant) methods
    just methods --live         # only Live methods
    just methods --json         # raw JSON pass-through for tooling

Exit code is always 0 — this is an audit surface, not a CI gate. The
gate is `scripts/check_method_registry.sh` (no flags), wired into
`just check`.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CHECK_SCRIPT = REPO_ROOT / "scripts" / "check_method_registry.sh"


def fetch_methods() -> list[dict]:
    """Shell to the check script in --list-json mode.

    Single parse source-of-truth — the bash script walks `src/ai/methods/`
    and emits the canonical record per method. Keeps Python out of the
    Rust-parsing business.
    """
    result = subprocess.run(
        ["bash", str(CHECK_SCRIPT), "--list-json"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout or "[]")


def format_text(methods: list[dict], filter_state: str | None) -> str:
    if filter_state:
        methods = [m for m in methods if m["state"].lower() == filter_state.lower()]

    if not methods:
        if filter_state == "PendingSubstrate":
            return "no dormant methods registered."
        if filter_state == "Live":
            return "no live methods registered."
        return "no methods registered."

    pending = [m for m in methods if m["state"] == "PendingSubstrate"]
    live = [m for m in methods if m["state"] == "Live"]

    lines: list[str] = []
    if live and filter_state != "PendingSubstrate":
        lines.append(f"Live ({len(live)}):")
        for m in live:
            lines.append(f"  {m['method_id']:<32}  {m['source']}")
        if pending and filter_state is None:
            lines.append("")

    if pending and filter_state != "Live":
        lines.append(f"PendingSubstrate ({len(pending)}):")
        for m in pending:
            blocker = m["blocker"]
            lines.append(
                f"  {m['method_id']:<32}  blocker {blocker:<6}  {m['source']}"
            )

    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "--pending",
        action="store_true",
        help="show only PendingSubstrate (dormant) methods",
    )
    group.add_argument(
        "--live",
        action="store_true",
        help="show only Live methods",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit raw JSON from the check script (pass-through)",
    )
    args = parser.parse_args()

    methods = fetch_methods()

    if args.json:
        print(json.dumps(methods, indent=2))
        return 0

    filter_state = None
    if args.pending:
        filter_state = "PendingSubstrate"
    elif args.live:
        filter_state = "Live"

    print(format_text(methods, filter_state))
    return 0


if __name__ == "__main__":
    sys.exit(main())
