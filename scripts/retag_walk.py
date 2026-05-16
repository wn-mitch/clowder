#!/usr/bin/env python3
"""Interactive per-ticket retag walk.

Companion to retag-suggest --apply (which batches): retag-walk yields one
ticket at a time and prompts the operator to accept / skip / accept-all / quit.
Useful for hand-curating the orchestration tags on a subset of the corpus
where the heuristic classifier needs human judgment per row.

Composes retag_suggest.py --json (for the heuristic) and retag.sh (for the
edit), so the heuristic stays in one place.

Usage:
    retag-walk                          # walk every would-change suggestion
    retag-walk --track swarm-safe       # walk only suggestions targeting one track
    retag-walk --include-noop           # also walk current==suggested rows
    retag-walk --dry-run                # print the actions without applying
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

VALID_TRACKS = ("substrate-sensitive", "coherent-block", "swarm-safe")


def gather_suggestions(track_filter: str | None) -> list[dict]:
    cmd = ["python3", str(REPO_ROOT / "scripts" / "retag_suggest.py"), "--json"]
    if track_filter:
        cmd += ["--only", track_filter]
    out = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True, check=False)
    if out.returncode != 0:
        print(f"ERROR: retag_suggest.py failed: {out.stderr}", file=sys.stderr)
        sys.exit(out.returncode or 1)
    return json.loads(out.stdout)


def apply_suggestion(ticket_id: str, track: str, block: str) -> int:
    cmd = ["bash", str(REPO_ROOT / "scripts" / "retag.sh"), ticket_id, "--track", track]
    if block:
        cmd += ["--block", block]
    return subprocess.run(cmd, cwd=REPO_ROOT, check=False).returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--track", choices=VALID_TRACKS,
                        help="walk only suggestions for this target track")
    parser.add_argument("--include-noop", action="store_true",
                        help="also walk rows where current == suggested")
    parser.add_argument("--dry-run", action="store_true",
                        help="print actions without applying")
    args = parser.parse_args()

    suggestions = gather_suggestions(args.track)
    if not args.include_noop:
        suggestions = [s for s in suggestions if s.get("would_change")]

    if not suggestions:
        print("retag-walk: nothing to walk (no would-change suggestions"
              + (f" for track '{args.track}'" if args.track else "") + ")")
        return 0

    print(f"retag-walk: {len(suggestions)} candidate(s)")
    print("  prompts: [y]es / [n]o / [a]ll-remaining / [q]uit\n")

    accept_all = False
    applied = 0
    skipped = 0

    for i, s in enumerate(suggestions, 1):
        if s.get("would_change"):
            change = f"{s['current'] or '(untagged)'} → {s['suggested']}"
        else:
            change = "(no-op)"
        print(f"[{i}/{len(suggestions)}] {s['id']}  {change}")
        print(f"    file:      {s['file']}")
        print(f"    current:   {s['current']}")
        print(f"    suggested: {s['suggested']}" + (f" (block: {s['block']})" if s.get("block") else ""))
        print(f"    reason:    {s['reason']}")

        if args.dry_run:
            print("    DRY-RUN — not applying\n")
            continue

        if accept_all:
            choice = "y"
        else:
            try:
                choice = input("    apply? [y/n/a/q] ").strip().lower()
            except EOFError:
                choice = "q"

        if choice in ("q", "quit"):
            print("retag-walk: quit by user")
            break
        if choice in ("a", "all"):
            accept_all = True
            choice = "y"
        if choice in ("y", "yes"):
            rc = apply_suggestion(s["id"], s["suggested"], s.get("block", ""))
            if rc != 0:
                print(f"    ERROR: retag.sh failed (exit {rc})", file=sys.stderr)
                skipped += 1
            else:
                applied += 1
                print("    APPLIED")
        else:
            skipped += 1
            print("    skipped")
        print()

    print(f"retag-walk: applied={applied} skipped={skipped}")
    if applied > 0 and not args.dry_run:
        # Regenerate the open-work index after batch edits.
        subprocess.run(["just", "open-work-index"], cwd=REPO_ROOT, check=False, capture_output=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
