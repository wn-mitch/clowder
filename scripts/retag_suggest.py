#!/usr/bin/env python3
"""Heuristic auto-classifier for orchestration retagging.

Walks every active ticket in docs/open-work/tickets/ and emits a
suggestion for which track + (if coherent-block) which block. The
/retag skill consumes the JSON output to drive the interactive walk.

Heuristic rules (in order of priority; first match wins):

    1. Ticket carries `wires-method:` frontmatter
       → coherent-block + block: htn-method-composition
    2. Ticket's blocked-by list contains 128 (HTN epic)
       → coherent-block + block: htn-method-composition
    3. cluster: tooling-diagnostics-ui | process-discipline
       AND filename matches docs/template/frontmatter/migration patterns
       → swarm-safe
    4. cluster: tooling-diagnostics-ui (general)
       → swarm-safe (most tooling work is mechanical / atomic)
    5. Body contains "[suspect]" or a layer-walk table marker
       → substrate-sensitive (no promotion)
    6. cluster in substrate-sensitive set
       → substrate-sensitive (default; explicit promote needed)
    7. Otherwise → substrate-sensitive (safe default)

Usage:
    retag_suggest.py [--only <track>] [--json]
    retag_suggest.py --apply       # apply suggestions via retag.sh

Output (default): human-readable table.
Output (--json):  array of {id, file, current, suggested, block, reason}.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

TICKETS_DIR = Path("docs/open-work/tickets")
RETAG_SH = "scripts/retag.sh"

VALID_TRACKS = {"substrate-sensitive", "coherent-block", "swarm-safe"}

HTN_EPIC_ID = "128"
HTN_BLOCK_NAME = "htn-method-composition"

SWARM_SAFE_CLUSTERS_GENERAL = {"tooling-diagnostics-ui", "process-discipline"}

SUBSTRATE_SENSITIVE_CLUSTERS = {
    "ai-substrate",
    "planner-and-steps",
    "combat-threat",
    "belief-perception",
    "social-coordination",
    "life-cycle",
    "items-crafting",
    "buildings-zones",
    "wildlife",
    "magic-mythic",
}

SUSPECT_MARKERS = ("[suspect]", "## Layer-walk", "layer-walk audit", "Hot context")
SWARM_FILENAME_HINTS = re.compile(r"(template|frontmatter|migration|index|cluster|wiki|landed-at-backfill)", re.IGNORECASE)


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


def classify(path: Path, fm: dict[str, str], body_sample: str) -> dict[str, str]:
    """Return {suggested_track, suggested_block, reason}."""
    cluster = fm.get("cluster", "").strip()
    blocked_by = parse_list(fm.get("blocked-by", "[]"))

    # Rule 1: wires-method ⇒ HTN block member
    if fm.get("wires-method", "").strip():
        return {
            "suggested": "coherent-block",
            "block": HTN_BLOCK_NAME,
            "reason": "wires-method: present (HTN method-registry glue)",
        }

    # Rule 2: blocked-by 128 (HTN epic) ⇒ HTN block member
    if HTN_EPIC_ID in [str(b).strip() for b in blocked_by]:
        return {
            "suggested": "coherent-block",
            "block": HTN_BLOCK_NAME,
            "reason": f"blocked-by: includes {HTN_EPIC_ID} (HTN epic)",
        }

    # Rule 3: tooling/process clusters + swarm-safe filename hint
    if cluster in SWARM_SAFE_CLUSTERS_GENERAL and SWARM_FILENAME_HINTS.search(path.name):
        return {
            "suggested": "swarm-safe",
            "block": "",
            "reason": f"cluster:{cluster} + filename hint",
        }

    # Rule 4: general tooling-diagnostics-ui (mostly atomic mechanical work)
    if cluster == "tooling-diagnostics-ui":
        return {
            "suggested": "swarm-safe",
            "block": "",
            "reason": "cluster:tooling-diagnostics-ui (default to swarm-safe; manually promote bugfixes to substrate-sensitive)",
        }

    # Rule 5: substrate-sensitive cues in body
    for marker in SUSPECT_MARKERS:
        if marker in body_sample:
            return {
                "suggested": "substrate-sensitive",
                "block": "",
                "reason": f"body contains '{marker}' (bugfix-discipline indicator)",
            }

    # Rule 6: substrate-sensitive clusters
    if cluster in SUBSTRATE_SENSITIVE_CLUSTERS:
        return {
            "suggested": "substrate-sensitive",
            "block": "",
            "reason": f"cluster:{cluster} default",
        }

    # Rule 7: fallback
    return {
        "suggested": "substrate-sensitive",
        "block": "",
        "reason": "no rule matched; safe default",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--only", choices=sorted(VALID_TRACKS),
                        help="filter output to suggestions of this track only")
    parser.add_argument("--json", action="store_true",
                        help="emit JSON array (used by /retag skill)")
    parser.add_argument("--apply", action="store_true",
                        help="apply suggestions via retag.sh (commits per-batch)")
    args = parser.parse_args()

    if not TICKETS_DIR.is_dir():
        print(f"ERROR: {TICKETS_DIR} not found", file=sys.stderr)
        return 2

    suggestions: list[dict[str, str]] = []

    for path in sorted(TICKETS_DIR.glob("*.md")):
        if path.name.startswith("_"):
            continue

        fm = parse_frontmatter(path)
        ticket_id = fm.get("id", "").strip()
        current = fm.get("orchestration", "").strip() or "—"

        # Read first ~200 lines of body as sample for content heuristics
        try:
            with path.open(encoding="utf-8") as fh:
                content = fh.read()
            body_start = content.find("\n---\n", 1)
            body_sample = content[body_start: body_start + 4000] if body_start > 0 else ""
        except OSError:
            body_sample = ""

        result = classify(path, fm, body_sample)

        suggestion = {
            "id": ticket_id,
            "file": str(path),
            "current": current,
            "suggested": result["suggested"],
            "block": result["block"],
            "reason": result["reason"],
            "would_change": current != result["suggested"] or (
                result["block"] and result["block"] not in parse_list(fm.get("initiative", "[]"))
            ),
        }

        if args.only and suggestion["suggested"] != args.only:
            continue

        suggestions.append(suggestion)

    if args.apply:
        applied = 0
        for s in suggestions:
            if not s["would_change"]:
                continue
            cmd = ["bash", RETAG_SH, s["id"], "--track", s["suggested"]]
            if s["block"]:
                cmd.extend(["--block", s["block"], "--initiative", s["block"]])
            try:
                subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL)
                applied += 1
            except subprocess.CalledProcessError as exc:
                print(f"FAIL: retag.sh {s['id']}: {exc}", file=sys.stderr)
        print(f"retag-suggest --apply: applied {applied} change(s) of {len(suggestions)} candidate(s)")
        return 0

    if args.json:
        print(json.dumps(suggestions, indent=2))
        return 0

    # Human table
    print(f"{'ID':<6} {'CURRENT':<22} {'SUGGESTED':<22} {'BLOCK':<26} REASON")
    for s in suggestions:
        mark = "→" if s["would_change"] else "·"
        print(f"{s['id']:<6} {s['current']:<22} {mark} {s['suggested']:<20} "
              f"{s['block'] or '—':<26} {s['reason']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
