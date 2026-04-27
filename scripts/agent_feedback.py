#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Friction-log breadcrumb writer.

Append a single JSON line to ``logs/agent-friction.jsonl`` capturing what an
agent (Claude or otherwise) tried, where it got stuck, and how severe the
friction was. Intended for the moment of friction — when a workflow bonks,
a tool's output didn't match its SKILL.md, or no `just` command serves the
user's intent.

The corpus accumulates over weeks of sessions. Periodic review surfaces
recurring patterns that warrant a CLAUDE.md update, a new SKILL.md, or
a new tool.

Usage:
    just agent-feedback "<note>"
    just agent-feedback "<note>" --severity major --tool verdict
    just agent-feedback "<note>" --what-tried "<...>" --where-stuck "<...>"
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
FRICTION_PATH = REPO_ROOT / "logs" / "agent-friction.jsonl"

SEVERITIES = ("minor", "major", "blocker")


def _commit_hash() -> str | None:
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, cwd=REPO_ROOT, timeout=5,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    out = proc.stdout.strip()
    return out or None


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("note", help="One-sentence summary of the friction.")
    ap.add_argument("--severity", choices=SEVERITIES, default="minor",
                    help="minor (annoyance) | major (re-do work) | blocker (couldn't proceed). "
                         "Default minor.")
    ap.add_argument("--tool", default=None,
                    help="Tool name the friction was around (e.g. 'q', 'verdict', "
                         "'frame-diff'). Optional but useful for filtering later.")
    ap.add_argument("--what-tried", default=None,
                    help="What the agent tried. Optional; the note often covers it.")
    ap.add_argument("--where-stuck", default=None,
                    help="Where things broke or became ambiguous. Optional.")
    args = ap.parse_args(argv)

    record = {
        "timestamp": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        "commit": _commit_hash(),
        "cwd": os.getcwd(),
        "note": args.note,
        "severity": args.severity,
        "tool": args.tool,
        "what_tried": args.what_tried,
        "where_stuck": args.where_stuck,
    }

    try:
        FRICTION_PATH.parent.mkdir(parents=True, exist_ok=True)
        with FRICTION_PATH.open("a") as f:
            f.write(json.dumps(record) + "\n")
    except OSError as e:
        sys.stderr.write(f"agent-feedback: failed to append to {FRICTION_PATH}: {e}\n")
        return 1

    sys.stderr.write(f"agent-feedback: recorded ({args.severity}) → {FRICTION_PATH}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
