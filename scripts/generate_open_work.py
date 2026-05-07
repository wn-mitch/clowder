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
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path


# ---------------------------------------------------------------------------
# Frontmatter parsing (minimal YAML subset — scalars, null, lists)
# ---------------------------------------------------------------------------


def _parse_scalar(value: str):
    value = value.strip()
    if value in ("null", "~", ""):
        return None
    if value in ("true", "True"):
        return True
    if value in ("false", "False"):
        return False
    # Flow-style list: [a, b, c] or []
    if value.startswith("[") and value.endswith("]"):
        inner = value[1:-1].strip()
        if not inner:
            return []
        return [_unquote(x.strip()) for x in inner.split(",") if x.strip()]
    # Try int
    try:
        return int(value)
    except ValueError:
        pass
    return _unquote(value)


def _unquote(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ('"', "'"):
        return value[1:-1]
    return value


def parse_frontmatter(text: str) -> dict:
    """Parse minimal YAML frontmatter at the top of a markdown file.

    Supports scalars, nulls, flow-style lists, and block-style lists:
        key: value
        key: null
        key: [a, b, c]
        key:
          - a
          - b
    """
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}
    end = None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            end = i
            break
    if end is None:
        return {}

    result: dict = {}
    current_list_key: str | None = None
    for raw in lines[1:end]:
        line = raw.rstrip()
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        # Continuation of block-style list
        stripped = line.lstrip()
        if current_list_key is not None and stripped.startswith("- "):
            result[current_list_key].append(_unquote(stripped[2:].strip()))
            continue
        current_list_key = None
        # key: value
        m = re.match(r"^([A-Za-z0-9_-]+):\s*(.*)$", line)
        if not m:
            continue
        key, value = m.group(1), m.group(2)
        if value == "":
            # Block-style list starts on next line
            result[key] = []
            current_list_key = key
        else:
            result[key] = _parse_scalar(value)
    return result


# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------


@dataclass
class Ticket:
    path: Path
    frontmatter: dict
    body: str

    @property
    def id(self) -> str:
        raw = self.frontmatter.get("id", "???")
        if isinstance(raw, int):
            return f"{raw:03d}"
        return str(raw)

    @property
    def title(self) -> str:
        return str(self.frontmatter.get("title", "(untitled)"))

    @property
    def status(self) -> str:
        return str(self.frontmatter.get("status", "ready"))

    @property
    def cluster(self):
        return self.frontmatter.get("cluster")

    @property
    def parked(self):
        return self.frontmatter.get("parked")

    @property
    def blocked_by(self) -> list:
        val = self.frontmatter.get("blocked-by") or []
        return val if isinstance(val, list) else [val]

    @property
    def added(self):
        return self.frontmatter.get("added")


def load_tickets(tickets_dir: Path) -> list[Ticket]:
    tickets = []
    if not tickets_dir.exists():
        return tickets
    for p in sorted(tickets_dir.glob("*.md")):
        if p.name.startswith("_") or p.name.lower() == "readme.md":
            continue
        text = p.read_text(encoding="utf-8")
        fm = parse_frontmatter(text)
        # Body starts after the closing --- of frontmatter (we don't need it
        # for the index, but keep it for future features).
        tickets.append(Ticket(path=p, frontmatter=fm, body=""))
    return tickets


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


def _format_id(raw) -> str:
    if isinstance(raw, int):
        return f"{raw:03d}"
    try:
        return f"{int(str(raw)):03d}"
    except (TypeError, ValueError):
        return str(raw)


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

# Match a markdown link target whose filename starts with NNN- (1-3 digits +
# optional single letter) and ends in .md. Anchored to `(` or `/` so we
# never grab the year prefix on date-named landed files (`2026-04-19-...`).
_CHILD_LINK_RE = re.compile(
    r"(?:\(|/)(\d{1,3}[a-z]?)-[A-Za-z0-9_-]+\.md(?=\))"
)

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


def _normalize_child_id(raw: str) -> str:
    """Pad numeric prefix to 3 digits, preserving any letter suffix
    (`27b` -> `027b`)."""
    m = re.match(r"^(\d+)([a-z]?)$", raw)
    if not m:
        return raw
    num, suffix = m.group(1), m.group(2)
    return f"{int(num):03d}{suffix}"


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


def build_ticket_index(repo_root: Path) -> dict[str, Ticket]:
    """Merge tickets/ + landed/ + pre-existing/ into a single id → Ticket map."""
    out: dict[str, Ticket] = {}
    for sub in ("tickets", "landed", "pre-existing"):
        d = repo_root / "docs" / "open-work" / sub
        for t in load_tickets(d):
            # First-write-wins: open tickets shadow landed if both exist
            # (shouldn't happen in practice — land_ticket.py moves files —
            # but `tickets/` order comes first so open wins by default).
            out.setdefault(t.id, t)
    return out


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
        "`just open-work-wip` · `just open-work-epics` · "
        "`just open-work-index` (regenerate this file)."
    )
    lines.append("")

    # Epic progress (between Summary and per-status sections)
    ticket_index = build_ticket_index(repo_root)
    epics = [
        compute_epic_progress(e, ticket_index)
        for e in discover_epics(repo_root)
    ]
    lines.extend(render_epic_progress_section(epics, repo_root))

    # Per-status sections
    for s in STATUS_ORDER:
        bucket = by_status.get(s, [])
        if not bucket:
            continue
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
