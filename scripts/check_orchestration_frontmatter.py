#!/usr/bin/env python3
"""Enforce the four orchestration-frontmatter invariants on active tickets.

Spec: CLAUDE.md §"Parallel-session orchestration" / ticket 354.

Every active ticket in docs/open-work/tickets/ must declare an
``orchestration:`` track. The track determines verdict cadence, session
lifetime, and polecat-eligibility — see
docs/workflow/parallel-sessions.md.

Invariants:
    1. ``orchestration:`` present, value in
       {substrate-sensitive, coherent-block, swarm-safe}
    2. ``coherent-block`` ⇒ ``block:`` present AND in ``initiative:`` list
    3. ≤1 ``verdict-anchor: true`` per ``block:`` value
    4. ``swarm-safe`` ⇒ no ``block:`` and no ``verdict-anchor:``

Landed tickets and ``_template*.md`` files are exempt.

Run via ``just check``.
"""
from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

TICKETS_DIR = Path("docs/open-work/tickets")
VALID_TRACKS = {"substrate-sensitive", "coherent-block", "swarm-safe"}


def parse_frontmatter(path: Path) -> dict[str, str]:
    """Return {key: raw_value_string} for the first YAML-ish frontmatter block.

    Treats `null`/empty as missing-from-the-caller's perspective by returning
    the raw string and letting callers handle it.
    """
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


def parse_initiative_list(raw: str) -> list[str]:
    """Parse ``[a, b]`` style YAML inline list into a Python list of strings."""
    raw = raw.strip()
    if not raw or raw == "[]":
        return []
    if raw.startswith("[") and raw.endswith("]"):
        inner = raw[1:-1].strip()
        if not inner:
            return []
        return [s.strip().strip('"').strip("'") for s in inner.split(",")]
    # tolerate bare scalar (uncommon but valid YAML)
    return [raw.strip().strip('"').strip("'")]


def main() -> int:
    if not TICKETS_DIR.is_dir():
        print(f"FAIL: {TICKETS_DIR} not found (run from repo root)", file=sys.stderr)
        return 2

    errors: list[str] = []
    seen = 0
    anchor_counts: dict[str, list[str]] = defaultdict(list)

    for ticket_path in sorted(TICKETS_DIR.glob("*.md")):
        if ticket_path.name.startswith("_"):
            continue
        seen += 1
        fm = parse_frontmatter(ticket_path)

        orchestration = fm.get("orchestration", "").strip()
        block = fm.get("block", "").strip()
        verdict_anchor = fm.get("verdict-anchor", "").strip()
        initiative_raw = fm.get("initiative", "[]")
        initiative = parse_initiative_list(initiative_raw)

        # Invariant 1
        if not orchestration:
            errors.append(f"{ticket_path}: missing 'orchestration:' (run 'just retag-init')")
            continue
        if orchestration not in VALID_TRACKS:
            errors.append(
                f"{ticket_path}: orchestration='{orchestration}' "
                f"(expected one of {sorted(VALID_TRACKS)})"
            )
            continue

        # Invariant 4: swarm-safe forbids block + verdict-anchor
        if orchestration == "swarm-safe":
            if block and block != "null":
                errors.append(
                    f"{ticket_path}: swarm-safe must not carry 'block:' (got '{block}')"
                )
            if verdict_anchor == "true":
                errors.append(
                    f"{ticket_path}: swarm-safe must not carry 'verdict-anchor: true'"
                )

        # Invariant 2: coherent-block requires block + must be in initiative list
        if orchestration == "coherent-block":
            if not block or block == "null":
                errors.append(f"{ticket_path}: coherent-block must declare 'block:'")
            elif block not in initiative:
                errors.append(
                    f"{ticket_path}: block:'{block}' must appear in "
                    f"initiative: {initiative_raw}"
                )

        # Tally for Invariant 3
        if verdict_anchor == "true":
            if not block or block == "null":
                errors.append(
                    f"{ticket_path}: verdict-anchor:true requires a 'block:' value"
                )
            else:
                anchor_counts[block].append(ticket_path.name)

    # Invariant 3: at most one anchor per block
    for block_name, owners in anchor_counts.items():
        if len(owners) > 1:
            errors.append(
                f"block '{block_name}' has {len(owners)} verdict-anchor:true "
                f"tickets (must be ≤1): {', '.join(owners)}"
            )

    if errors:
        for err in errors:
            print(f"ERROR: {err}")
        print(f"\nFAIL: {len(errors)} orchestration-frontmatter violation(s) "
              f"across {seen} ticket(s)")
        return 1

    print(f"orchestration-frontmatter: OK ({seen} ticket(s) validated)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
