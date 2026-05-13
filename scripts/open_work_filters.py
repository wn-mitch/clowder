#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Filter / projection views over docs/open-work/tickets/ + landed/ frontmatter.

Backs the `just open-work-active`, `just open-work-ready --cluster/--initiative`,
`just open-work-stale`, `just open-work-blocking`, and `just initiatives` recipes.

Frontmatter parsing is shared with generate_open_work.py via direct import —
single source of truth for what fields mean.

Usage:
    open_work_filters.py active
    open_work_filters.py ready [--cluster <name>] [--initiative <name>]
    open_work_filters.py stale [--days N]
    open_work_filters.py blocking <id>
    open_work_filters.py initiatives
"""

from __future__ import annotations

import argparse
import datetime as dt
import sys
from collections import defaultdict
from pathlib import Path

# Re-use the canonical frontmatter parser + Ticket dataclass.
SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))
from generate_open_work import (  # type: ignore[import-not-found]
    Ticket,
    _format_id,
    build_ticket_index,
    compute_blocker_reverse_index,
    fetch_next_top_k,
    load_tickets,
)

REPO_ROOT = SCRIPTS_DIR.parent
TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"
LANDED_DIR = REPO_ROOT / "docs" / "open-work" / "landed"


def _print_ticket(t: Ticket) -> None:
    bits: list[str] = []
    if t.cluster:
        bits.append(f"[{t.cluster}]")
    if t.initiative:
        bits.append(f"⟨{', '.join(t.initiative)}⟩")
    suffix = " " + " ".join(bits) if bits else ""
    print(f"  {t.id:>5}  {t.title}{suffix}")


def cmd_active() -> int:
    """Mirror the `## Active focus` projection from open-work.md."""
    tickets = load_tickets(TICKETS_DIR)
    by_id = {t.id: t for t in tickets}
    in_progress = sorted([t for t in tickets if t.status == "in-progress"], key=lambda t: t.id)

    # Transitive blockers of active work, restricted to ready status.
    in_progress_ids = {t.id for t in in_progress}
    active_ids = set(in_progress_ids)
    frontier = list(in_progress_ids)
    while frontier:
        cur = frontier.pop()
        cur_t = by_id.get(cur)
        if not cur_t:
            continue
        for bid in cur_t.blocked_by:
            fid = _format_id(bid)
            if fid not in active_ids and fid in by_id:
                active_ids.add(fid)
                frontier.append(fid)
    ready_blockers = sorted(
        [
            by_id[i] for i in active_ids
            if i in by_id and by_id[i].status == "ready" and i not in in_progress_ids
        ],
        key=lambda t: t.id,
    )

    if in_progress:
        print(f"In progress ({len(in_progress)}):")
        for t in in_progress:
            _print_ticket(t)
    if ready_blockers:
        print(f"\nReady — blocking active work ({len(ready_blockers)}):")
        for t in ready_blockers:
            _print_ticket(t)

    next_results = fetch_next_top_k(5)
    if next_results:
        print(f"\nNext-recommended (top 5 from `just next`):")
        for r in next_results:
            tid = _format_id(r.get("id", "?"))
            title = r.get("title", "(untitled)")
            cluster = r.get("cluster") or "—"
            score = r.get("score")
            score_str = f" · {score:.2f}" if isinstance(score, (int, float)) else ""
            print(f"  {tid:>5}  {title} [{cluster}]{score_str}")

    if not in_progress and not ready_blockers and not next_results:
        print("(no active work — corpus is empty or all tickets are parked/blocked)")
    return 0


def cmd_ready(cluster: str | None, initiative: str | None) -> int:
    """List ready tickets, optionally filtered by cluster or initiative."""
    tickets = load_tickets(TICKETS_DIR)
    ready = sorted([t for t in tickets if t.status == "ready"], key=lambda t: t.id)

    if cluster:
        ready = [t for t in ready if t.cluster == cluster]
    if initiative:
        ready = [t for t in ready if initiative in t.initiative]

    filter_bits = []
    if cluster:
        filter_bits.append(f"cluster={cluster}")
    if initiative:
        filter_bits.append(f"initiative={initiative}")
    suffix = f" ({', '.join(filter_bits)})" if filter_bits else ""
    print(f"Ready ({len(ready)}){suffix}:")
    for t in ready:
        _print_ticket(t)
    return 0


def cmd_stale(days: int) -> int:
    """List parked tickets older than `days` days. Missing `parked:` dates
    print as 'undated' — they're equally suspect."""
    tickets = load_tickets(TICKETS_DIR)
    parked = [t for t in tickets if t.status == "parked"]
    today = dt.date.today()
    threshold = today - dt.timedelta(days=days)

    stale: list[tuple[Ticket, dt.date | None]] = []
    undated: list[Ticket] = []
    for t in parked:
        raw = t.parked
        if isinstance(raw, str):
            try:
                d = dt.date.fromisoformat(raw)
                if d <= threshold:
                    stale.append((t, d))
            except ValueError:
                undated.append(t)
        else:
            undated.append(t)

    stale.sort(key=lambda pair: pair[1] or dt.date.min)
    print(f"Stale parked (≥{days}d old, threshold {threshold.isoformat()}): {len(stale)}")
    for t, d in stale:
        age = (today - d).days if d else "?"
        print(f"  {t.id:>5}  parked {d} ({age}d)  {t.title}")
    if undated:
        print(f"\nParked but undated ({len(undated)}) — needs backfill:")
        for t in undated:
            _print_ticket(t)
    return 0


def cmd_blocking(seed_id: str) -> int:
    """Show all tickets that transitively block the given ticket."""
    tickets = load_tickets(TICKETS_DIR)
    by_id = {t.id: t for t in tickets}
    target_id = _format_id(seed_id)
    if target_id not in by_id:
        print(f"open-work-filters: ticket {target_id} not found in tickets/", file=sys.stderr)
        return 1

    seen: set[str] = set()
    chain: list[Ticket] = []
    frontier = [target_id]
    while frontier:
        cur = frontier.pop()
        t = by_id.get(cur)
        if not t:
            continue
        for bid in t.blocked_by:
            fid = _format_id(bid)
            if fid in seen or fid not in by_id:
                continue
            seen.add(fid)
            chain.append(by_id[fid])
            frontier.append(fid)
    target = by_id[target_id]
    print(f"Blockers of [{target_id}] {target.title} ({len(chain)} transitively):")
    for t in sorted(chain, key=lambda t: t.id):
        status_marker = "✓" if t.status == "done" else ("⊙" if t.status == "ready" else "⊘")
        print(f"  {status_marker} {t.id:>5}  [{t.status}] {t.title}")
    return 0


def cmd_initiatives() -> int:
    """List active initiatives with open / landed counts."""
    open_tix = load_tickets(TICKETS_DIR)
    landed_tix = load_tickets(LANDED_DIR) if LANDED_DIR.exists() else []
    init_open: dict[str, int] = defaultdict(int)
    init_landed: dict[str, int] = defaultdict(int)
    for t in open_tix:
        for i in t.initiative:
            init_open[str(i)] += 1
    for t in landed_tix:
        for i in t.initiative:
            init_landed[str(i)] += 1
    all_inits = set(init_open) | set(init_landed)
    if not all_inits:
        print("(no initiatives tagged yet)")
        return 0
    print(f"Initiatives ({len(all_inits)}):")
    for init in sorted(all_inits, key=lambda i: (-init_open[i], i)):
        print(f"  {init:<28}  open={init_open[init]:>3}  landed={init_landed[init]:>3}")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("active")
    p_ready = sub.add_parser("ready")
    p_ready.add_argument("--cluster", default=None)
    p_ready.add_argument("--initiative", default=None)
    p_stale = sub.add_parser("stale")
    p_stale.add_argument("--days", type=int, default=30)
    p_blocking = sub.add_parser("blocking")
    p_blocking.add_argument("id")
    sub.add_parser("initiatives")

    args = ap.parse_args(argv)
    if args.cmd == "active":
        return cmd_active()
    if args.cmd == "ready":
        return cmd_ready(args.cluster, args.initiative)
    if args.cmd == "stale":
        return cmd_stale(args.days)
    if args.cmd == "blocking":
        return cmd_blocking(args.id)
    if args.cmd == "initiatives":
        return cmd_initiatives()
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
