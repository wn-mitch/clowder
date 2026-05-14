#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Open-work index generator for Clowder.

Walks `docs/open-work/tickets/`, `docs/open-work/pre-existing/`, and
`docs/open-work/landed/`, parses frontmatter, and emits a scannable
`docs/open-work.md` index grouped by status. The per-file tickets are the
source of truth; this index is a derived view.

Usage:
    uv run scripts/generate_open_work.py
    uv run scripts/generate_open_work.py --out docs/open-work.md
"""

import argparse
import datetime as dt
import json
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

# Shared frontmatter / index helpers (also used by epic_children.py).
sys.path.insert(0, str(Path(__file__).resolve().parent))
from _ticket_frontmatter import (  # noqa: E402
    Ticket,
    _CHILD_LINK_RE,
    _format_id,
    _normalize_child_id,
    build_ticket_index,
    load_tickets,
    parse_frontmatter,
)


# ---------------------------------------------------------------------------
# Index rendering
# ---------------------------------------------------------------------------


STATUS_ORDER = ["in-progress", "ready", "parked", "blocked", "dropped", "done"]

STATUS_LABEL = {
    "in-progress": "In progress",
    "ready": "Ready",
    "parked": "Parked",
    "blocked": "Blocked",
    "dropped": "Dropped",
    "done": "Done (awaiting archive)",
}

# Open-status buckets used for the "open" rollup in epic progress and the
# index summary. `done` is excluded — epic progress treats it as "shipped",
# the index summary excludes it from `Open total`.
OPEN_STATUSES = ("in-progress", "ready", "parked", "blocked")


# ---------------------------------------------------------------------------
# Epic progress
# ---------------------------------------------------------------------------
#
# An "epic" is a ticket file whose name ends in `-epic.md`. Epics own a
# roster of child tickets. We derive their progress by:
#   1. Locating an `## Open child tickets` (or `### ...`) section in the
#      epic body. If absent, scan the whole body — covers phased epics that
#      reference children inline (e.g. 095).
#   2. Extracting markdown-link references that point at sibling ticket
#      files: `(NNN-slug.md)` or `(../landed/NNN-slug.md)`. The numeric
#      prefix (with optional letter, e.g. `027b`) is the child id.
#   3. Looking each child id up in the merged tickets+landed+pre-existing
#      index (frontmatter id is source of truth). Unknown ids are dropped.


_EPIC_FILENAME_SUFFIX = "-epic.md"

_ROSTER_HEADING_RE = re.compile(
    r"^(#{2,6})\s+open child tickets\b", re.IGNORECASE
)


@dataclass
class EpicProgress:
    epic: Ticket
    roster_kind: str  # "explicit" (Open child tickets section) or "inline"
    children: list[Ticket] = field(default_factory=list)
    missing_ids: list[str] = field(default_factory=list)

    @property
    def total(self) -> int:
        return len(self.children)

    @property
    def done(self) -> int:
        return sum(1 for c in self.children if c.status == "done")

    @property
    def open_count(self) -> int:
        return sum(1 for c in self.children if c.status in OPEN_STATUSES)

    @property
    def status_counts(self) -> dict[str, int]:
        out: dict[str, int] = {}
        for c in self.children:
            out[c.status] = out.get(c.status, 0) + 1
        return out

    @property
    def percent_done(self) -> int:
        if self.total == 0:
            return 0
        return round(100 * self.done / self.total)


def _extract_roster_segment(body: str) -> tuple[str, str]:
    """Return (segment, kind). Kind is `"explicit"` if an `Open child tickets`
    heading is found, else `"inline"` and the segment is the whole body."""
    lines = body.splitlines()
    start: int | None = None
    start_level = 0
    for i, line in enumerate(lines):
        m = _ROSTER_HEADING_RE.match(line)
        if m:
            start = i + 1
            start_level = len(m.group(1))
            break
    if start is None:
        return body, "inline"
    end = len(lines)
    for j in range(start, len(lines)):
        m = re.match(r"^(#{1,6})\s", lines[j])
        if m and len(m.group(1)) <= start_level:
            end = j
            break
    return "\n".join(lines[start:end]), "explicit"


def discover_epics(repo_root: Path) -> list[Ticket]:
    """Return all epics across tickets/ and landed/, sorted by id."""
    epics: list[Ticket] = []
    for sub in ("tickets", "landed"):
        d = repo_root / "docs" / "open-work" / sub
        if not d.exists():
            continue
        for p in sorted(d.glob(f"*{_EPIC_FILENAME_SUFFIX}")):
            text = p.read_text(encoding="utf-8")
            fm = parse_frontmatter(text)
            body = text.split("---", 2)[-1] if text.startswith("---") else text
            epics.append(Ticket(path=p, frontmatter=fm, body=body))
    epics.sort(key=lambda t: t.id)
    return epics


def compute_epic_progress(
    epic: Ticket, ticket_index: dict[str, Ticket]
) -> EpicProgress:
    segment, kind = _extract_roster_segment(epic.body)
    seen: set[str] = set()
    children: list[Ticket] = []
    missing: list[str] = []
    self_id = epic.id
    for m in _CHILD_LINK_RE.finditer(segment):
        cid = _normalize_child_id(m.group(1))
        if cid == self_id or cid in seen:
            continue
        seen.add(cid)
        child = ticket_index.get(cid)
        if child is None:
            missing.append(cid)
            continue
        children.append(child)
    children.sort(key=lambda t: t.id)
    return EpicProgress(epic=epic, roster_kind=kind, children=children, missing_ids=missing)


def render_ticket_line(t: Ticket, repo_root: Path) -> str:
    rel = t.path.relative_to(repo_root)
    bits = []
    if t.cluster:
        bits.append(f"[{t.cluster}]")
    if t.parked:
        bits.append(f"parked {t.parked}")
    if t.blocked_by:
        bits.append("blocked-by " + ", ".join(_format_id(b) for b in t.blocked_by))
    if t.added:
        bits.append(f"added {t.added}")
    suffix = f" — _{' · '.join(bits)}_" if bits else ""
    return f"- **[{t.id}]({rel})** — {t.title}{suffix}"


def compute_blocker_reverse_index(tickets: list[Ticket]) -> dict[str, set[str]]:
    """Map each ticket id → set of ids that have it in their `blocked-by`.

    Used to find ready tickets that unblock active work: if T's id appears in
    some active ticket's blocker set, T is on the critical path.
    """
    reverse: dict[str, set[str]] = defaultdict(set)
    for t in tickets:
        for bid in t.blocked_by:
            reverse[_format_id(bid)].add(t.id)
    return reverse


def fetch_next_top_k(k: int = 5) -> list[dict]:
    """Shell out to `just next` and return top-K results, or [] on failure.

    `just next` writes JSON to stdout (envelope shape from logq). We tolerate
    non-zero exit (e.g. embedding index missing or `just` unavailable) by
    returning [] so the section silently degrades.
    """
    try:
        result = subprocess.run(
            ["just", "next", "--top", str(k)],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode != 0:
            return []
        envelope = json.loads(result.stdout)
        return envelope.get("results", [])[:k]
    except (subprocess.SubprocessError, json.JSONDecodeError, FileNotFoundError):
        return []


def render_active_focus_section(
    tickets: list[Ticket],
    repo_root: Path,
) -> list[str]:
    """`## Active focus` — what's loadbearing right now.

    Composed of three slices: in-progress tickets, ready tickets that unblock
    active work, and the top-5 recommendation from `just next`. Tickets that
    appear in multiple slices are deduplicated by id with the first slice
    winning (in-progress > blocker-of-active > just-next).
    """
    by_status: dict[str, list[Ticket]] = defaultdict(list)
    for t in tickets:
        by_status[t.status].append(t)
    by_id = {t.id: t for t in tickets}

    in_progress = sorted(by_status.get("in-progress", []), key=lambda t: t.id)
    in_progress_ids = {t.id for t in in_progress}

    # Build set of active-relevant ticket ids: in-progress + anything they
    # transitively depend on (blocked-by chain among open tickets).
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

    # Ready blockers-of-active = ready tickets whose id is in active_ids and
    # which are not themselves in-progress.
    ready_blockers = sorted(
        (
            by_id[i]
            for i in active_ids
            if i in by_id
            and by_id[i].status == "ready"
            and i not in in_progress_ids
        ),
        key=lambda t: t.id,
    )

    next_results = fetch_next_top_k(5)

    if not in_progress and not ready_blockers and not next_results:
        return []

    lines: list[str] = []
    lines.append(
        f"## Active focus ({len(in_progress)} in-progress · "
        f"{len(ready_blockers)} ready blockers · {len(next_results)} next-recommended)"
    )
    lines.append("")
    lines.append(
        "Auto-generated projection: what's load-bearing right now. "
        "In-progress tickets, ready tickets that unblock active work, and "
        "the top-5 from `just next`. See `## Ready by cluster` / "
        "`## Ready by initiative` below for the full queue."
    )
    lines.append("")

    seen: set[str] = set()

    if in_progress:
        lines.append("### In progress")
        lines.append("")
        for t in in_progress:
            lines.append(render_ticket_line(t, repo_root))
            seen.add(t.id)
        lines.append("")

    fresh_blockers = [t for t in ready_blockers if t.id not in seen]
    if fresh_blockers:
        lines.append("### Ready — blocking active work")
        lines.append("")
        for t in fresh_blockers:
            lines.append(render_ticket_line(t, repo_root))
            seen.add(t.id)
        lines.append("")

    fresh_next = [r for r in next_results if str(r.get("id", "")) not in seen]
    if fresh_next:
        lines.append("### Next-recommended (from `just next`)")
        lines.append("")
        for r in fresh_next:
            rid = _format_id(r.get("id", "???"))
            title = r.get("title", "(untitled)")
            cluster = r.get("cluster") or "—"
            path = r.get("path", "")
            bits = f"[{cluster}]"
            score = r.get("score")
            if score is not None:
                bits += f" · score {score:.2f}"
            lines.append(f"- **[{rid}]({path})** — {title} — _{bits}_")
            seen.add(rid)
        lines.append("")

    return lines


def render_ready_by_cluster_section(
    ready_tickets: list[Ticket],
    blocked_tickets: list[Ticket],
    repo_root: Path,
) -> list[str]:
    """`## Ready by cluster` — cluster-major rendering of the ready queue.

    Sort clusters by ready count descending; emit "Uncategorized" last
    regardless of size so the gap is always the bottom-of-section signal.
    Blocked count per cluster annotates the heading.
    """
    if not ready_tickets:
        return []

    ready_by_cluster: dict[str, list[Ticket]] = defaultdict(list)
    for t in ready_tickets:
        key = t.cluster or "__uncategorized__"
        ready_by_cluster[key].append(t)

    blocked_by_cluster: dict[str, int] = defaultdict(int)
    for t in blocked_tickets:
        key = t.cluster or "__uncategorized__"
        blocked_by_cluster[key] += 1

    # Sort: count descending, then alphabetical; Uncategorized always last.
    named = [k for k in ready_by_cluster if k != "__uncategorized__"]
    named.sort(key=lambda k: (-len(ready_by_cluster[k]), k))
    cluster_order = named + (["__uncategorized__"] if "__uncategorized__" in ready_by_cluster else [])

    lines: list[str] = []
    lines.append(f"## Ready by cluster ({len(ready_tickets)})")
    lines.append("")
    lines.append(
        "Cluster = categorical bucket (one per ticket). See "
        "`docs/open-work/clusters.md` for the taxonomy. The Uncategorized "
        "count is the actionable signal — tickets without a cluster don't "
        "show up in `just open-work-ready --cluster X` filters."
    )
    lines.append("")

    for key in cluster_order:
        bucket = sorted(ready_by_cluster[key], key=lambda t: t.id)
        if key == "__uncategorized__":
            heading_name = "Uncategorized"
        else:
            heading_name = key
        blocked_n = blocked_by_cluster.get(key, 0)
        bits = f"{len(bucket)} ready"
        if blocked_n:
            bits += f", {blocked_n} blocked"
        lines.append(f"### {heading_name} ({bits})")
        lines.append("")
        for t in bucket:
            lines.append(render_ticket_line(t, repo_root))
        lines.append("")

    return lines


def render_ready_by_initiative_section(
    ready_tickets: list[Ticket],
    landed_tickets: list[Ticket],
    repo_root: Path,
) -> list[str]:
    """`## Ready by initiative` — thematic rollups across cluster lines.

    Tickets without an `initiative:` tag are omitted — they appear under
    `## Ready by cluster`. Tickets carrying multiple initiatives appear in
    every matching subsection.
    """
    open_by_init: dict[str, list[Ticket]] = defaultdict(list)
    for t in ready_tickets:
        for init in t.initiative:
            open_by_init[str(init)].append(t)

    if not open_by_init:
        return []

    landed_by_init: dict[str, int] = defaultdict(int)
    for t in landed_tickets:
        for init in t.initiative:
            landed_by_init[str(init)] += 1

    # Sort: open count desc, then alphabetical.
    init_order = sorted(open_by_init.keys(), key=lambda k: (-len(open_by_init[k]), k))

    total_tagged = sum(len(v) for v in open_by_init.values())
    lines: list[str] = []
    lines.append(f"## Ready by initiative ({total_tagged} tag-memberships across {len(open_by_init)} initiatives)")
    lines.append("")
    lines.append(
        "Initiative = thematic outcome (zero-or-more per ticket). Tickets "
        "may appear in multiple subsections. Tickets without any initiative "
        "tag are omitted here and visible under `## Ready by cluster`. See "
        "`docs/open-work/initiatives/` for outcome definitions."
    )
    lines.append("")

    for init in init_order:
        bucket = sorted(open_by_init[init], key=lambda t: t.id)
        landed_n = landed_by_init.get(init, 0)
        lines.append(f"### {init} ({len(bucket)} open, {landed_n} landed)")
        lines.append("")
        for t in bucket:
            lines.append(render_ticket_line(t, repo_root))
        lines.append("")

    return lines


def render_epic_progress_section(
    epics: list[EpicProgress], repo_root: Path
) -> list[str]:
    """Render the `## Epic progress` section as markdown lines."""
    if not epics:
        return []
    lines: list[str] = []
    lines.append(f"## Epic progress ({len(epics)})")
    lines.append("")
    lines.append(
        "Per-epic completion derived from each epic's roster table "
        "(or inline child references for phased epics)."
    )
    lines.append("")
    lines.append("| Epic | Status | Children | Done | Open (in-progress / ready / blocked / parked) | Progress |")
    lines.append("|---|---|---|---|---|---|")
    for ep in epics:
        rel = ep.epic.path.relative_to(repo_root)
        title_short = ep.epic.title.split(" — ")[0]
        if ep.total == 0:
            progress_cell = "_no roster_" if ep.roster_kind == "inline" else "_empty roster_"
            children_cell = "0"
            done_cell = "—"
            open_cell = "—"
        else:
            counts = ep.status_counts
            ip = counts.get("in-progress", 0)
            rd = counts.get("ready", 0)
            bl = counts.get("blocked", 0)
            pk = counts.get("parked", 0)
            children_cell = str(ep.total)
            done_cell = f"{ep.done}"
            open_cell = f"{ep.open_count} ({ip} / {rd} / {bl} / {pk})"
            filled = round(ep.percent_done / 10)
            bar = "▰" * filled + "▱" * (10 - filled)
            progress_cell = f"`{bar}` {ep.percent_done}%"
        lines.append(
            f"| **[{ep.epic.id}]({rel})** {title_short} | {ep.epic.status} | "
            f"{children_cell} | {done_cell} | {open_cell} | {progress_cell} |"
        )
    lines.append("")
    # Surface any missing-child references — usually a stale link in the
    # epic body that didn't survive a rename. Easier to fix than to debug.
    stale = [(ep.epic.id, mid) for ep in epics for mid in ep.missing_ids]
    if stale:
        lines.append("> **Missing child references** (link in epic body but no matching ticket file):")
        for eid, mid in stale:
            lines.append(f"> - epic {eid} → `{mid}`")
        lines.append("")
    return lines


def render_index(
    tickets: list[Ticket],
    pre_existing: list[Ticket],
    landed_dir: Path,
    repo_root: Path,
) -> str:
    today = dt.date.today().isoformat()

    # Group tickets by status
    by_status: dict[str, list[Ticket]] = {s: [] for s in STATUS_ORDER}
    for t in tickets:
        by_status.setdefault(t.status, []).append(t)

    # Sort each bucket: by id numeric if possible, else string
    def _sort_key(t: Ticket):
        raw = t.frontmatter.get("id")
        if isinstance(raw, int):
            return (0, raw)
        try:
            return (0, int(str(raw)))
        except (TypeError, ValueError):
            return (1, str(raw))

    for s in by_status:
        by_status[s].sort(key=_sort_key)

    lines: list[str] = []
    lines.append("# Open work")
    lines.append("")
    lines.append(
        "<!-- AUTO-GENERATED by scripts/generate_open_work.py — do not edit by hand. -->"
    )
    lines.append(
        "<!-- Source of truth: docs/open-work/tickets/*.md, docs/open-work/pre-existing/*.md. -->"
    )
    lines.append("")
    lines.append(
        "> **What this is:** the cross-thread index of open work. New sessions should"
    )
    lines.append(
        "> consult this, `docs/wiki/systems.md`, and `docs/balance/*.md` before starting"
    )
    lines.append(
        "> fresh. See `CLAUDE.md` §\"Long-horizon coordination\" for the request-time"
    )
    lines.append("> checklist and maintenance rules.")
    lines.append("")
    lines.append(f"_Last generated: {today}._")
    lines.append("")

    # Summary table
    lines.append("## Summary")
    lines.append("")
    lines.append("| Status | Count |")
    lines.append("|---|---|")
    total_open = 0
    for s in STATUS_ORDER:
        n = len(by_status.get(s, []))
        if s in ("in-progress", "ready", "parked", "blocked"):
            total_open += n
        if n:
            lines.append(f"| {STATUS_LABEL[s]} | {n} |")
    lines.append(f"| **Open total** | **{total_open}** |")
    lines.append(f"| Pre-existing | {len(pre_existing)} |")
    lines.append("")
    lines.append(
        "Source of truth: one markdown file per entry under "
        "`docs/open-work/{tickets,pre-existing}/`. Landing archive: "
        "`docs/open-work/landed/`."
    )
    lines.append("")
    lines.append(
        "Queue-view commands: `just open-work` · `just open-work-ready` · "
        "`just open-work-wip` · `just open-work-active` · "
        "`just open-work-epics` · `just open-work-index` (regenerate)."
    )
    lines.append("")

    # Active focus projection (in-progress + ready blockers + just next top-5)
    lines.extend(render_active_focus_section(tickets, repo_root))

    # Epic progress (between Summary/Active focus and per-status sections)
    ticket_index = build_ticket_index(repo_root)
    epics = [
        compute_epic_progress(e, ticket_index)
        for e in discover_epics(repo_root)
    ]
    lines.extend(render_epic_progress_section(epics, repo_root))

    # Load landed once for initiative rollups.
    landed_tickets_for_rollup = (
        load_tickets(landed_dir) if landed_dir.exists() else []
    )

    # Per-status sections. Before the flat `## Ready (N)` list, emit the
    # cluster-major and initiative-major projections so the queue is
    # navigable by category before falling back to id order.
    for s in STATUS_ORDER:
        bucket = by_status.get(s, [])
        if not bucket:
            continue
        if s == "ready":
            lines.extend(
                render_ready_by_cluster_section(
                    bucket, by_status.get("blocked", []), repo_root
                )
            )
            lines.extend(
                render_ready_by_initiative_section(
                    bucket, landed_tickets_for_rollup, repo_root
                )
            )
        lines.append(f"## {STATUS_LABEL[s]} ({len(bucket)})")
        lines.append("")
        for t in bucket:
            lines.append(render_ticket_line(t, repo_root))
        lines.append("")

    # Pre-existing
    if pre_existing:
        lines.append(f"## Pre-existing ({len(pre_existing)})")
        lines.append("")
        for t in sorted(pre_existing, key=lambda x: x.id):
            rel = t.path.relative_to(repo_root)
            lines.append(f"- **[{t.id}]({rel})** — {t.title}")
        lines.append("")

    # Landed archive — per-ticket files grouped by year-month (most recent first)
    if landed_dir.exists():
        landed_tickets = load_tickets(landed_dir)
        if landed_tickets:
            by_month: dict[str, list[Ticket]] = {}
            for lt in landed_tickets:
                month = "unknown"
                landed_on = lt.frontmatter.get("landed-on")
                if isinstance(landed_on, str) and len(landed_on) >= 7:
                    month = landed_on[:7]
                by_month.setdefault(month, []).append(lt)

            lines.append(f"## Landed archive ({len(landed_tickets)})")
            lines.append("")
            lines.append(
                f"Full history: [`docs/open-work/landed/`]("
                f"{landed_dir.relative_to(repo_root)}/)."
            )
            lines.append("")
            for month in sorted(by_month.keys(), reverse=True):
                bucket = sorted(
                    by_month[month],
                    key=lambda x: (x.frontmatter.get("landed-on") or "", x.id),
                    reverse=True,
                )
                lines.append(f"### {month} ({len(bucket)})")
                lines.append("")
                for lt in bucket:
                    rel = lt.path.relative_to(repo_root)
                    landed_on = lt.frontmatter.get("landed-on") or "?"
                    lines.append(
                        f"- **[{lt.id}]({rel})** — {lt.title} _({landed_on})_"
                    )
                lines.append("")

    # Conventions footer
    lines.append("## Conventions")
    lines.append("")
    lines.append(
        "- **Opening a ticket:** create `docs/open-work/tickets/NNN-slug.md` "
        "with `status: ready`."
    )
    lines.append(
        "- **Picking up work:** flip `status: in-progress`, regenerate the index, "
        "commit together with first code change."
    )
    lines.append(
        "- **Landing:** set `status: done`, `landed-at: <sha>`, `landed-on: <date>`, "
        "move file to `docs/open-work/landed/YYYY-MM.md` (merge as an `## ` entry), "
        "regenerate the index, commit."
    )
    lines.append(
        "- **Parking:** set `status: parked`, `parked: <date>`, leave in place. "
        "Add a `## Log` entry explaining why."
    )
    lines.append(
        "- **Blocking:** set `status: blocked`, populate `blocked-by: [ids]`. "
        "The blocking ticket should reference it via `## Log`."
    )
    lines.append(
        "- **Every landing commit** regenerates this file via `just open-work-index`."
    )
    lines.append("")

    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="Path to the clowder repo root (default: parent of scripts/).",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Output file (default: <repo>/docs/open-work.md).",
    )
    args = parser.parse_args()

    repo_root = args.repo.resolve()
    tickets_dir = repo_root / "docs" / "open-work" / "tickets"
    pre_existing_dir = repo_root / "docs" / "open-work" / "pre-existing"
    landed_dir = repo_root / "docs" / "open-work" / "landed"
    out_path = args.out or (repo_root / "docs" / "open-work.md")

    tickets = load_tickets(tickets_dir)
    pre_existing = load_tickets(pre_existing_dir)

    rendered = render_index(tickets, pre_existing, landed_dir, repo_root)
    out_path.write_text(rendered, encoding="utf-8")

    print(f"Wrote {out_path.relative_to(repo_root)}")
    print(f"  tickets:       {len(tickets)}")
    print(f"  pre-existing:  {len(pre_existing)}")
    if landed_dir.exists():
        landed = load_tickets(landed_dir)
        print(f"  landed:        {len(landed)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
