#!/usr/bin/env python3
"""Single-ticket frontmatter + status + holding-session view.

Stage 1.5 of ticket 354. Composes the orchestration axis + session
state into one structured read for the /work skill.

Usage:
    ticket_info.py <id> [--json]
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from _ticket_frontmatter import load_tickets  # noqa: E402

TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"
LANDED_DIR = REPO_ROOT / "docs" / "open-work" / "landed"
SESSIONS_ROOT = Path.home() / "clowder-sessions"


def find_holding_session(ticket_id: str) -> str | None:
    """Return the session slug holding this ticket (in-progress), or None."""
    if not SESSIONS_ROOT.is_dir():
        return None
    normalized = re.sub(r"^0+", "", str(ticket_id)) or "0"
    for sd in SESSIONS_ROOT.iterdir():
        info = sd / ".session-info.json"
        if not info.exists():
            continue
        try:
            data = json.loads(info.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        tickets = [re.sub(r"^0+", "", str(t)) or "0" for t in data.get("tickets", [])]
        if normalized in tickets:
            return data.get("slug", sd.name)
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("id", help="ticket id (with or without leading zeros)")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    requested = re.sub(r"^0+", "", str(args.id)) or "0"

    # Search active + landed
    found = None
    where = None
    for src, label in ((TICKETS_DIR, "active"), (LANDED_DIR, "landed")):
        if not src.is_dir():
            continue
        for t in load_tickets(src):
            if (re.sub(r"^0+", "", t.id) or "0") == requested:
                found = t
                where = label
                break
        if found:
            break

    if not found:
        print(f"ERROR: no ticket matches id '{args.id}'", file=sys.stderr)
        return 1

    session = find_holding_session(requested) if where == "active" else None

    data = {
        "id": found.id,
        "title": found.title,
        "status": found.status,
        "where": where,
        "cluster": found.cluster,
        "initiative": found.initiative,
        "orchestration": found.orchestration,
        "block": found.block,
        "verdict_anchor": found.verdict_anchor,
        "blocked_by": found.blocked_by,
        "path": str(found.path.relative_to(REPO_ROOT)),
        "holding_session": session,
    }

    if args.json:
        print(json.dumps(data, indent=2))
        return 0

    print(f"ticket {data['id']}: {data['title']}")
    print(f"  status:         {data['status']}  ({where})")
    print(f"  cluster:        {data['cluster']}")
    print(f"  orchestration:  {data['orchestration']}")
    if data["block"]:
        print(f"  block:          {data['block']}{'  ★ verdict-anchor' if data['verdict_anchor'] else ''}")
    if data["initiative"]:
        print(f"  initiative:     {data['initiative']}")
    if data["blocked_by"]:
        print(f"  blocked-by:     {data['blocked_by']}")
    if session:
        print(f"  held by:        session/{session}")
    print(f"  path:           {data['path']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
