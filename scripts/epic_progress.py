#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Epic progress tracker for Clowder.

Walks every ticket under `docs/open-work/tickets/` (and the landed mirror),
selects the ones that are epics — filename ends in `-epic.md`, or
frontmatter carries `epic: true` — parses each epic's roster section for
child-ticket references, and reports per-epic completion: total children,
done, in-progress, ready, blocked, parked.

Source of truth: each epic's `## Open child tickets — full roster` table
(or, for phased epics without a roster, inline child references in the body).
Child status is read from each child's frontmatter.

Usage:
    uv run scripts/epic_progress.py                   # summary table for all epics
    uv run scripts/epic_progress.py --epic 093        # detail for one epic
    uv run scripts/epic_progress.py --detailed        # per-child rows for every epic
    uv run scripts/epic_progress.py --json            # machine-readable
"""

import argparse
import datetime as dt
import json
import sys
from pathlib import Path

# Reuse the shared epic-discovery / progress-computation logic from the
# index generator so the CLI view and the embedded `docs/open-work.md`
# section stay in lockstep.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_open_work import (  # noqa: E402
    EpicProgress,
    OPEN_STATUSES,
    build_ticket_index,
    compute_epic_progress,
    discover_epics,
)
from _ticket_frontmatter import load_tickets  # noqa: E402


# Status badges for terminal output.
STATUS_BADGE = {
    "in-progress": "WIP",
    "ready": "RDY",
    "blocked": "BLK",
    "parked": "PRK",
    "done": "DONE",
    "dropped": "DRP",
}


def _format_bar(pct: int, width: int = 10) -> str:
    filled = round(pct / (100 / width))
    return "▰" * filled + "▱" * (width - filled)


def _format_one_line(ep: EpicProgress) -> list[str]:
    out: list[str] = []
    title = ep.epic.title
    out.append(f"{ep.epic.id}  [{ep.epic.status}]  {title}")
    if ep.total == 0:
        if ep.roster_kind == "inline":
            out.append("       no roster — phased epic, work scoped inline in body")
        else:
            out.append("       empty roster")
        return out
    counts = ep.status_counts
    parts = []
    for s in ("in-progress", "ready", "blocked", "parked", "done"):
        n = counts.get(s, 0)
        if n:
            parts.append(f"{n} {s}")
    open_total = ep.open_count
    out.append(
        f"       {ep.total} children · "
        f"{ep.done} done · {open_total} open"
    )
    out.append(
        f"       {_format_bar(ep.percent_done)} "
        f"{ep.percent_done}%   ({', '.join(parts) if parts else 'empty'})"
    )
    return out


def _format_detailed(ep: EpicProgress) -> list[str]:
    out: list[str] = _format_one_line(ep)
    if ep.total == 0:
        return out
    out.append("")
    for child in ep.children:
        badge = STATUS_BADGE.get(child.status, child.status[:4].upper())
        bits = []
        if child.blocked_by:
            bits.append("blocked-by " + ", ".join(str(b) for b in child.blocked_by))
        if child.parked:
            bits.append(f"parked {child.parked}")
        suffix = f"  ({'; '.join(bits)})" if bits else ""
        out.append(f"       {badge:>4}  {child.id}  {child.title}{suffix}")
    if ep.missing_ids:
        out.append("")
        out.append("       Missing child references in roster:")
        for mid in ep.missing_ids:
            out.append(f"              - {mid}  (no ticket file resolves to this id)")
    return out


def _epic_to_dict(ep: EpicProgress) -> dict:
    return {
        "id": ep.epic.id,
        "title": ep.epic.title,
        "status": ep.epic.status,
        "path": str(ep.epic.path),
        "roster_kind": ep.roster_kind,
        "total": ep.total,
        "done": ep.done,
        "open": ep.open_count,
        "percent_done": ep.percent_done,
        "status_counts": ep.status_counts,
        "missing_ids": ep.missing_ids,
        "children": [
            {
                "id": c.id,
                "title": c.title,
                "status": c.status,
                "blocked_by": [str(b) for b in c.blocked_by],
                "path": str(c.path),
            }
            for c in ep.children
        ],
    }


def _run_lint(epics: list[EpicProgress], repo_root: Path, stale_days: int) -> int:
    """Lint check: flag orphan tickets + stale epic rosters.

    Orphan: a ready/in-progress ticket whose cluster matches an active
    epic's cluster but is not listed in that epic's roster.

    Stale: an in-progress epic whose file mtime is older than `stale_days`
    days (roster hasn't been updated; may be missing recently-opened tickets).

    Returns 0 if no issues, 1 if violations found.
    """
    today = dt.date.today()
    threshold_ts = (today - dt.timedelta(days=stale_days)).timetuple()
    import time
    threshold_epoch = time.mktime(threshold_ts)

    violations: list[str] = []

    tickets_dir = repo_root / "docs" / "open-work" / "tickets"
    open_tickets = load_tickets(tickets_dir)

    for ep in epics:
        if ep.epic.status not in ("in-progress", "ready"):
            continue

        epic_cluster = ep.epic.cluster

        # Rule 1: Orphan tickets — open tickets in this epic's cluster
        # that are not listed in the roster.
        if epic_cluster:
            roster_ids = {c.id for c in ep.children}
            for t in open_tickets:
                if t.status not in ("in-progress", "ready"):
                    continue
                if t.cluster != epic_cluster:
                    continue
                if t.id in roster_ids:
                    continue
                violations.append(
                    f"ORPHAN  [{t.id}] {t.title!r} has cluster={epic_cluster} "
                    f"but is not in epic [{ep.epic.id}] roster"
                )

        # Rule 2: Stale roster — epic file mtime older than threshold.
        epic_mtime = ep.epic.path.stat().st_mtime
        if epic_mtime < threshold_epoch and ep.open_count > 0:
            age_days = int((today.toordinal() -
                            dt.date.fromtimestamp(epic_mtime).toordinal()))
            violations.append(
                f"STALE   epic [{ep.epic.id}] {ep.epic.title!r} "
                f"roster last touched {age_days}d ago "
                f"({ep.open_count} open children)"
            )

    if violations:
        print(f"epic-rollup lint: {len(violations)} violation(s)")
        for v in violations:
            print(f"  {v}")
        return 1

    print(f"epic-rollup lint: OK (checked {len(epics)} epic(s))")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="Path to the clowder repo root (default: parent of scripts/).",
    )
    parser.add_argument(
        "--epic",
        type=str,
        default=None,
        help="Filter to one epic id (e.g. 060, 093, 095).",
    )
    parser.add_argument(
        "--detailed",
        action="store_true",
        help="Print every child ticket under each epic, not just the rollup.",
    )
    parser.add_argument(
        "--json",
        dest="emit_json",
        action="store_true",
        help="Emit JSON to stdout instead of human-readable output.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help=("Epic-rollup lint: flag open tickets not in any matching epic roster "
              "(orphans) and active epics with stale rosters (>30d untouched)."),
    )
    parser.add_argument(
        "--stale-days",
        type=int,
        default=30,
        metavar="N",
        help="Stale threshold in days for --check (default: 30).",
    )
    args = parser.parse_args()

    repo_root = args.repo.resolve()
    ticket_index = build_ticket_index(repo_root)
    epics = [compute_epic_progress(e, ticket_index) for e in discover_epics(repo_root)]

    if args.check:
        return _run_lint(epics, repo_root, args.stale_days)

    if args.epic:
        wanted = args.epic
        try:
            wanted = f"{int(wanted):03d}"
        except ValueError:
            pass
        epics = [e for e in epics if e.epic.id == wanted]
        if not epics:
            print(f"no epic matches id {args.epic}", file=sys.stderr)
            return 1

    if args.emit_json:
        print(json.dumps([_epic_to_dict(e) for e in epics], indent=2))
        return 0

    if not epics:
        print("no epics found")
        return 0

    formatter = _format_detailed if args.detailed else _format_one_line
    for i, ep in enumerate(epics):
        if i:
            print()
        for line in formatter(ep):
            print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
