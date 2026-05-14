#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
`just epic-children <id-or-path>` — child-ticket drift audit for an epic
dashboard ticket (default target: 060).

Why this exists
---------------
Epic-style tracker tickets maintain a hand-curated roster of child tickets
with `status` / `blocked-by` claims that can fall out of sync with each
child's own frontmatter. The dashboard's Anti-staleness Measure rule —
"child file is the truth" — is **unenforced** in the absence of a query
tool. This script:

  1. Locates the epic file (by id under `tickets/` or `landed/`, or by
     explicit path).
  2. Parses its `### Open child tickets` roster table.
  3. Looks each child id up under `docs/open-work/{tickets,landed,
     pre-existing}/`.
  4. Classifies drift per row (7 kinds; see DRIFT_KINDS).
  5. Emits a `logq`-style JSON envelope (or a human summary with `--text`).
  6. Optionally rewrites the roster table to match frontmatter
     (`--fix`).

Exit codes:
  0 — every roster row is consistent
  1 — drift detected (or rewritten under `--fix`)
  2 — epic not found / unparseable

Friction precedent: `logs/agent-friction.jsonl` 2026-05-14 (severity major).
Ticket: 318.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from _ticket_frontmatter import (  # noqa: E402
    Ticket,
    _CHILD_LINK_RE,
    _format_id,
    _normalize_child_id,
    build_ticket_index_with_landed,
)

TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"
LANDED_DIR = REPO_ROOT / "docs" / "open-work" / "landed"


# ---------------------------------------------------------------------------
# Drift kinds (canonical enum surfaced in `results[].drift_kind`)
# ---------------------------------------------------------------------------

DRIFT_KINDS = (
    "consistent",
    "landed-but-marked-active",
    "landed-but-sha-stale",
    "blocker-mismatch",
    "status-mismatch",
    "missing-file",
    "link-mismatch",
    "unparseable-status",
)

# Drift kinds that `--fix` will mechanically rewrite. The two excluded
# kinds — `missing-file` and `link-mismatch` — name ambiguous targets;
# the right fix is editorial (delete the row, or correct the link).
FIXABLE_DRIFT_KINDS = frozenset(
    {
        "landed-but-marked-active",
        "landed-but-sha-stale",
        "blocker-mismatch",
        "status-mismatch",
    }
)


# ---------------------------------------------------------------------------
# Epic file resolution
# ---------------------------------------------------------------------------


def resolve_epic_path(arg: str) -> Path:
    """Resolve `arg` to an epic file path.

    `arg` may be:
      - a bare numeric id (e.g. `60` or `060`) → search `tickets/` then
        `landed/` for the unique `<id>-*.md` (zero-padded or not).
      - a relative or absolute path → return as-is if it exists.

    Raises SystemExit(2) if no match or multiple matches.
    """
    p = Path(arg)
    if p.exists():
        return p.resolve()
    # Treat as an id. Tolerate non-padded ids (`60` → `060`).
    try:
        n = int(arg)
        candidates_padded = f"{n:03d}"
    except ValueError:
        raise SystemExit(
            f"epic-children: '{arg}' is neither an existing path nor a numeric id"
        )
    matches: list[Path] = []
    for d in (TICKETS_DIR, LANDED_DIR):
        if not d.exists():
            continue
        matches.extend(sorted(d.glob(f"{candidates_padded}-*.md")))
        if str(n) != candidates_padded:
            matches.extend(sorted(d.glob(f"{n}-*.md")))
    # De-dup
    seen: set[Path] = set()
    deduped: list[Path] = []
    for m in matches:
        if m not in seen:
            seen.add(m)
            deduped.append(m)
    if not deduped:
        raise SystemExit(
            f"epic-children: no epic file found for id={arg} in "
            f"{TICKETS_DIR.relative_to(REPO_ROOT)} or "
            f"{LANDED_DIR.relative_to(REPO_ROOT)}"
        )
    if len(deduped) > 1:
        names = ", ".join(p.name for p in deduped)
        raise SystemExit(
            f"epic-children: multiple files match id={arg}: {names}"
        )
    return deduped[0]


# ---------------------------------------------------------------------------
# Roster parser
# ---------------------------------------------------------------------------


_ROSTER_HEADING_RE = re.compile(
    r"^#{2,6}\s+open child tickets\b", re.IGNORECASE
)

_ROSTER_ROW_RE = re.compile(
    r"^\|\s*\[(?P<id>\d+[a-z]?)\]\((?P<link>[^)]+)\)\s*\|"
    r"\s*(?P<status>[^|]*?)\s*\|"
    r"\s*(?P<spec>[^|]*?)\s*\|"
    r"\s*(?P<scope>[^|]*?)\s*\|\s*$"
)

# Patterns for the status cell. Order matters — `^ready` would partial-match
# inside other strings, so each is anchored to the trimmed cell text.
_STATUS_READY_RE = re.compile(r"^ready$", re.IGNORECASE)
_STATUS_PARKED_RE = re.compile(
    r"^parked(?:\s*[—–\-]\s*(?P<note>.*))?$", re.IGNORECASE
)
_STATUS_BLOCKED_RE = re.compile(
    r"^blocked-by\s+(?P<blocker>\d+[a-z]?)\b", re.IGNORECASE
)
# `✅ landed (sha)` — sha is 6+ lowercase hex (matches the `jj log -T
# commit_id.short()` form used by `land_ticket.py:apply_sha_backfill`).
_STATUS_LANDED_RE = re.compile(
    r"^✅\s*landed(?:\s*\((?P<sha>[a-f0-9]+)\))?$", re.IGNORECASE
)
_STATUS_IN_FLIGHT_RE = re.compile(
    r"^\U0001F504\s*in\s*flight$", re.IGNORECASE
)


@dataclass
class RosterRow:
    line_index: int  # zero-based index into the file's line list
    raw_line: str
    id: str  # normalized 3-digit padded
    raw_id: str  # as it appeared in the link text
    link_path: str
    status_cell: str
    spec_cell: str
    scope_cell: str
    # Parsed status
    claim_status: str  # one of: ready | parked | blocked | landed | in-progress | ?
    claim_blocker: str | None  # normalized blocker id, when claim_status == blocked
    claim_sha: str | None  # short sha, when claim_status == landed and rendered
    claim_parked_note: str | None  # trailing " — note" on parked rows, preserved verbatim


@dataclass
class SectionRef:
    id: str
    section: str  # short slug describing where the reference lives
    claim_status: str | None  # best-effort: only set when the line carries one


def _tokenize_status(status_cell: str) -> tuple[str, str | None, str | None, str | None]:
    """Return (status_kind, blocker_id, sha, parked_note)."""
    s = status_cell.strip()
    if _STATUS_READY_RE.match(s):
        return ("ready", None, None, None)
    m = _STATUS_PARKED_RE.match(s)
    if m:
        return ("parked", None, None, (m.group("note") or "").strip() or None)
    m = _STATUS_BLOCKED_RE.match(s)
    if m:
        return ("blocked", _normalize_child_id(m.group("blocker")), None, None)
    m = _STATUS_LANDED_RE.match(s)
    if m:
        return ("landed", None, m.group("sha"), None)
    if _STATUS_IN_FLIGHT_RE.match(s):
        return ("in-progress", None, None, None)
    return ("?", None, None, None)


def parse_roster(epic_lines: list[str]) -> tuple[list[RosterRow], int | None, int | None]:
    """Locate the roster table; return (rows, header_idx, end_idx).

    Returns (rows, None, None) if no table is found.
    """
    start: int | None = None
    for i, line in enumerate(epic_lines):
        if _ROSTER_HEADING_RE.match(line):
            start = i
            break
    if start is None:
        return [], None, None

    # Scan forward to the header row `| Ticket | Status | ...`. Tolerate
    # blank lines between heading and table.
    header_idx: int | None = None
    for j in range(start + 1, len(epic_lines)):
        line = epic_lines[j].strip()
        if not line:
            continue
        # First non-blank should be the header. If it's not a `|`-row,
        # the table is shaped wrong; bail.
        if line.startswith("|") and "ticket" in line.lower() and "status" in line.lower():
            header_idx = j
        break

    if header_idx is None:
        return [], start, None

    # Skip header + separator
    body_start = header_idx + 1
    if (
        body_start < len(epic_lines)
        and epic_lines[body_start].lstrip().startswith("|")
        and "---" in epic_lines[body_start]
    ):
        body_start += 1

    rows: list[RosterRow] = []
    end_idx = body_start
    for k in range(body_start, len(epic_lines)):
        line = epic_lines[k]
        stripped = line.strip()
        if not stripped:
            end_idx = k
            break
        # Stop at next heading or any non-table-row (e.g. `**Total open: 33**`).
        if stripped.startswith("#") or not stripped.startswith("|"):
            end_idx = k
            break
        m = _ROSTER_ROW_RE.match(line)
        if not m:
            # A `|`-prefixed line that doesn't match — likely a malformed row.
            # Surface it as unparseable rather than silently skip.
            rows.append(
                RosterRow(
                    line_index=k,
                    raw_line=line,
                    id="???",
                    raw_id="",
                    link_path="",
                    status_cell=stripped,
                    spec_cell="",
                    scope_cell="",
                    claim_status="?",
                    claim_blocker=None,
                    claim_sha=None,
                    claim_parked_note=None,
                )
            )
            continue
        raw_id = m.group("id")
        normalized_id = _normalize_child_id(raw_id)
        status_text = m.group("status")
        kind, blocker, sha, note = _tokenize_status(status_text)
        rows.append(
            RosterRow(
                line_index=k,
                raw_line=line,
                id=normalized_id,
                raw_id=raw_id,
                link_path=m.group("link"),
                status_cell=status_text,
                spec_cell=m.group("spec"),
                scope_cell=m.group("scope"),
                claim_status=kind,
                claim_blocker=blocker,
                claim_sha=sha,
                claim_parked_note=note,
            )
        )
        end_idx = k + 1
    return rows, header_idx, end_idx


def collect_section_refs(
    epic_lines: list[str], roster_ids: set[str], roster_span: tuple[int, int] | None
) -> list[SectionRef]:
    """Find every `[NNN](NNN-...md)`-style reference outside the roster span.

    `roster_span` is (start_line, end_line_exclusive) for the table body —
    matches inside that range are roster rows and excluded.
    """
    refs: list[SectionRef] = []
    current_section = "preamble"
    section_start = 0
    span_start, span_end = roster_span if roster_span else (-1, -1)
    for i, line in enumerate(epic_lines):
        if line.startswith("## ") or line.startswith("### "):
            current_section = _slug_for_heading(line)
            section_start = i
        if span_start <= i < span_end:
            continue
        for m in _CHILD_LINK_RE.finditer(line):
            tid = _normalize_child_id(m.group(1))
            if tid in roster_ids:
                # The same id may legitimately appear in both the roster
                # AND Phase coverage map / prose — surface the outside
                # mentions but tag them as informational rather than drift.
                pass
            refs.append(
                SectionRef(
                    id=tid,
                    section=current_section,
                    claim_status=_extract_inline_status(line),
                )
            )
    return refs


def _slug_for_heading(heading: str) -> str:
    """Map a heading line like `### Phase coverage map` → `phase-coverage-map`."""
    text = re.sub(r"^#+\s*", "", heading).strip().lower()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"\s+", "-", text)
    return text or "section"


def _extract_inline_status(line: str) -> str | None:
    """Best-effort: pluck an `✅ landed` / `🔄 in flight` / `💤 parked` marker
    from a free-form line."""
    if "✅ landed" in line.lower() or "✅ landed" in line:
        return "landed"
    if "\U0001F504 in flight" in line or "🔄 in flight" in line:
        return "in-progress"
    if "\U0001F4A4" in line or "💤 parked" in line:
        return "parked"
    return None


# ---------------------------------------------------------------------------
# Drift classifier
# ---------------------------------------------------------------------------


@dataclass
class DriftResult:
    id: str
    row_line: int  # 1-based for human display
    dashboard_claim: str
    dashboard_blocker: str | None
    dashboard_sha: str | None
    frontmatter_status: str | None
    frontmatter_blocked_by: list[str]
    in_landed: bool
    landed_at: str | None
    drift_kind: str
    note: str | None = None


def classify_row(
    row: RosterRow,
    index: dict[str, tuple[Ticket | None, Ticket | None]],
) -> DriftResult:
    active, landed = index.get(row.id, (None, None))

    base = DriftResult(
        id=row.id,
        row_line=row.line_index + 1,
        dashboard_claim=row.claim_status,
        dashboard_blocker=row.claim_blocker,
        dashboard_sha=row.claim_sha,
        frontmatter_status=(active.status if active else (landed.status if landed else None)),
        frontmatter_blocked_by=[
            _format_id(b) for b in (active.blocked_by if active else [])
        ],
        in_landed=landed is not None,
        landed_at=(
            str(landed.frontmatter.get("landed-at")) if landed else None
        ),
        drift_kind="consistent",
    )

    # 0. Unparseable: parser flagged a malformed cell.
    if row.claim_status == "?" and row.raw_id == "":
        base.drift_kind = "unparseable-status"
        base.note = "row did not match the four-column roster shape"
        return base

    # 1. missing-file: neither active nor landed file exists.
    if active is None and landed is None:
        base.drift_kind = "missing-file"
        base.note = f"no file matches id={row.id} under tickets/ or landed/"
        return base

    # 2. link-mismatch: link path's leading id doesn't match the bracketed id.
    link_id_match = re.match(r"^(\d+[a-z]?)-", row.link_path)
    if link_id_match:
        link_id = _normalize_child_id(link_id_match.group(1))
        if link_id != row.id:
            base.drift_kind = "link-mismatch"
            base.note = (
                f"link text says [{row.raw_id}] but path starts with "
                f"{link_id_match.group(1)}-…; cannot disambiguate intended ticket"
            )
            return base

    # 3. landed-but-marked-active: child has landed; dashboard says otherwise.
    if landed is not None and row.claim_status != "landed":
        base.drift_kind = "landed-but-marked-active"
        if active is not None:
            base.note = (
                "dual-file: both tickets/ and landed/ have this id; "
                "landed/ wins for the claim"
            )
        return base

    # 4. landed-but-sha-stale: dashboard says landed; sha missing or wrong.
    if landed is not None and row.claim_status == "landed":
        actual_sha = landed.frontmatter.get("landed-at")
        if isinstance(actual_sha, str) and actual_sha not in (
            "pending",
            "null",
            "",
        ):
            if row.claim_sha is None:
                base.drift_kind = "landed-but-sha-stale"
                base.note = (
                    f"dashboard omits sha; frontmatter landed-at={actual_sha}"
                )
                return base
            if row.claim_sha.lower() != actual_sha.lower():
                base.drift_kind = "landed-but-sha-stale"
                base.note = (
                    f"dashboard says sha={row.claim_sha}; "
                    f"frontmatter landed-at={actual_sha}"
                )
                return base
        # If frontmatter has no sha (pending / null), claim is acceptable.

    # 5. status-mismatch: active ticket exists and statuses disagree.
    if active is not None:
        fm_status = active.status
        if not _statuses_match(row.claim_status, fm_status):
            base.drift_kind = "status-mismatch"
            base.note = (
                f"dashboard says {row.claim_status!r}; "
                f"frontmatter status={fm_status!r}"
            )
            return base

        # 6. blocker-mismatch: statuses agree, blocker id doesn't.
        if row.claim_status == "blocked":
            fm_blockers = base.frontmatter_blocked_by
            if not fm_blockers:
                base.drift_kind = "blocker-mismatch"
                base.note = (
                    f"dashboard says blocked-by {row.claim_blocker}; "
                    f"frontmatter blocked-by is empty"
                )
                return base
            fm_first = fm_blockers[0]
            if row.claim_blocker is None or _normalize_child_id(
                row.claim_blocker
            ) != fm_first:
                base.drift_kind = "blocker-mismatch"
                base.note = (
                    f"dashboard says blocked-by {row.claim_blocker}; "
                    f"frontmatter first blocker is {fm_first}"
                )
                return base

    return base


def _statuses_match(dashboard_kind: str, frontmatter_status: str) -> bool:
    """Compare dashboard status keyword to frontmatter `status:` value.

    Both are normalized to the dashboard's vocabulary so that `blocked` (the
    parsed kind from `blocked-by NNN`) maps to frontmatter `blocked`.
    """
    if dashboard_kind == "?":
        return False
    if dashboard_kind == frontmatter_status:
        return True
    # Treat `landed` as equivalent to `done` (the frontmatter field stays
    # `done` even after `land_ticket.py` runs; landed-ness lives in the
    # directory + `landed-at` field).
    if dashboard_kind == "landed" and frontmatter_status == "done":
        return True
    return False


# ---------------------------------------------------------------------------
# Envelope
# ---------------------------------------------------------------------------


@dataclass
class Envelope:
    query: dict[str, Any]
    scan_stats: dict[str, Any]
    results: list[dict[str, Any]] = field(default_factory=list)
    section_refs: list[dict[str, Any]] = field(default_factory=list)
    narrative: str = ""
    next: list[str] = field(default_factory=list)


def _display_path(p: Path) -> str:
    """Render a path relative to the repo root when possible, else absolute.

    Out-of-repo paths (tmp files used in test runs) shouldn't crash the
    envelope renderer just because they're outside the repo subtree.
    """
    try:
        return str(p.relative_to(REPO_ROOT))
    except ValueError:
        return str(p)


def render_envelope(
    epic_path: Path,
    epic_id: str,
    epic_arg: str,
    rows: list[RosterRow],
    drifts: list[DriftResult],
    section_refs: list[SectionRef],
    fix: bool,
    fixed_count: int,
) -> Envelope:
    drift_count = sum(1 for d in drifts if d.drift_kind != "consistent")
    env = Envelope(
        query={
            "epic": epic_id,
            "path": _display_path(epic_path),
            "fix": fix,
        },
        scan_stats={
            "roster_entries": len(rows),
            "section_refs": len(section_refs),
            "drift_detected": drift_count,
            "fixed": fixed_count,
        },
    )
    for d in drifts:
        env.results.append(
            {
                "id": d.id,
                "row_line": d.row_line,
                "dashboard_claim": d.dashboard_claim,
                "dashboard_blocker": d.dashboard_blocker,
                "dashboard_sha": d.dashboard_sha,
                "frontmatter_status": d.frontmatter_status,
                "frontmatter_blocked_by": d.frontmatter_blocked_by,
                "in_landed": d.in_landed,
                "landed_at": d.landed_at,
                "drift_kind": d.drift_kind,
                "note": d.note,
            }
        )
    for r in section_refs:
        env.section_refs.append(
            {"id": r.id, "section": r.section, "claim_status": r.claim_status}
        )
    env.narrative = _build_narrative(rows, drifts, fixed_count, fix)
    env.next = _suggest_next(drifts, epic_arg, fix)
    return env


def _build_narrative(
    rows: list[RosterRow],
    drifts: list[DriftResult],
    fixed_count: int,
    fix: bool,
) -> str:
    total = len(rows)
    drift_count = sum(1 for d in drifts if d.drift_kind != "consistent")
    if fix and fixed_count:
        return (
            f"{fixed_count}/{drift_count} drift row(s) rewritten; "
            f"{total - drift_count} consistent on entry."
        )
    if drift_count == 0:
        return f"{total}/{total} roster entries consistent. No drift detected."
    # Tally per kind for the human summary.
    by_kind: dict[str, int] = {}
    for d in drifts:
        if d.drift_kind == "consistent":
            continue
        by_kind[d.drift_kind] = by_kind.get(d.drift_kind, 0) + 1
    parts = ", ".join(f"{n} {k}" for k, n in sorted(by_kind.items()))
    return f"{drift_count}/{total} drift row(s): {parts}."


def _suggest_next(drifts: list[DriftResult], epic_arg: str, fix: bool) -> list[str]:
    if fix:
        return []
    suggestions: list[str] = []
    fixable = any(d.drift_kind in FIXABLE_DRIFT_KINDS for d in drifts)
    if fixable:
        suggestions.append(f"just epic-children {epic_arg} --fix")
    if any(d.drift_kind == "missing-file" for d in drifts):
        suggestions.append(
            "edit the dashboard by hand to remove or correct missing-file rows"
        )
    if any(d.drift_kind == "link-mismatch" for d in drifts):
        suggestions.append(
            "edit the dashboard by hand to fix link-mismatch rows (ambiguous auto-fix)"
        )
    return suggestions


# ---------------------------------------------------------------------------
# `--fix` mutator
# ---------------------------------------------------------------------------


def apply_fix(
    epic_path: Path,
    epic_lines: list[str],
    rows: list[RosterRow],
    drifts: list[DriftResult],
    index: dict[str, tuple[Ticket | None, Ticket | None]],
    today: str,
) -> tuple[int, list[tuple[str, str]]]:
    """Rewrite roster rows in-place. Returns (count_fixed, [(id, drift_kind)...])."""
    fixable_by_line: dict[int, DriftResult] = {}
    by_id: dict[str, RosterRow] = {r.id: r for r in rows}
    for d in drifts:
        if d.drift_kind in FIXABLE_DRIFT_KINDS:
            row = by_id.get(d.id)
            if row is None:
                continue
            fixable_by_line[row.line_index] = d

    if not fixable_by_line:
        return 0, []

    fixed_records: list[tuple[str, str]] = []
    for line_idx, drift in fixable_by_line.items():
        row = next(r for r in rows if r.line_index == line_idx)
        active, landed = index.get(row.id, (None, None))
        new_status_text = _render_new_status_cell(row, active, landed)
        new_line = _replace_status_cell(epic_lines[line_idx], new_status_text)
        epic_lines[line_idx] = new_line
        fixed_records.append((row.id, drift.drift_kind))

    epic_lines = _append_log_entry(epic_lines, today, fixed_records)
    epic_path.write_text("\n".join(epic_lines) + "\n", encoding="utf-8")
    return len(fixed_records), fixed_records


def _render_new_status_cell(
    row: RosterRow, active: Ticket | None, landed: Ticket | None
) -> str:
    """Produce the corrected Status-cell text for a roster row.

    Authoritative source:
      - landed/ wins when both files exist.
      - otherwise, active/ frontmatter.
    """
    if landed is not None:
        sha = landed.frontmatter.get("landed-at")
        if isinstance(sha, str) and sha not in ("pending", "null", ""):
            return f"✅ landed ({sha})"
        return "✅ landed"
    if active is not None:
        status = active.status
        if status == "ready":
            return "ready"
        if status == "in-progress":
            return "\U0001F504 in flight"
        if status == "parked":
            # Preserve any prior "— note" suffix when frontmatter is still parked.
            note = row.claim_parked_note
            if note:
                return f"parked — {note}"
            return "parked"
        if status == "blocked":
            first = next(iter(active.blocked_by), None)
            if first is None:
                # Frontmatter says blocked but blocked-by is empty — the
                # row stays blocked but without a referent. Fall back to
                # the prior cell text so we don't fabricate a blocker id.
                return "blocked"
            return f"blocked-by {_format_id(first)}"
        # Defensive: any other status (e.g. `done` without a landed move,
        # `dropped`) is surfaced verbatim.
        return status
    # No file at all — caller shouldn't have routed this through --fix.
    return row.status_cell.strip()


def _replace_status_cell(line: str, new_status_text: str) -> str:
    """Rewrite the second cell of a 4-column markdown table row.

    Preserves the surrounding pipes and the leading/trailing space inside
    the cell; only the trimmed cell content changes. The Ticket / Spec /
    Scope cells are byte-preserved.
    """
    parts = line.split("|")
    # A well-formed row line splits into ['', ' Ticket ', ' Status ',
    # ' Spec ', ' Scope ', ''] — 6 parts. Skip mutation if shape is off.
    if len(parts) < 6:
        return line
    parts[2] = f" {new_status_text} "
    return "|".join(parts)


def _append_log_entry(
    lines: list[str], today: str, fixed: list[tuple[str, str]]
) -> list[str]:
    """Append a `- <today>: epic-children --fix ...` line under `## Log`.

    Mirrors `land_ticket.py:append_log_entry` but inlined to avoid coupling
    epic-children to land_ticket's internals.
    """
    if not fixed:
        return lines
    # Truncate the per-row list at 10 ids to keep the line readable.
    rendered: list[str] = []
    for tid, kind in fixed[:10]:
        rendered.append(f"{tid} {kind}")
    if len(fixed) > 10:
        rendered.append(f"… +{len(fixed) - 10} more")
    summary = "; ".join(rendered)
    entry = (
        f"- {today}: `epic-children --fix` touched {len(fixed)} roster row(s) "
        f"({summary}). Auto-generated by scripts/epic_children.py."
    )

    log_idx = next(
        (i for i, line in enumerate(lines) if line.strip() == "## Log"),
        None,
    )
    if log_idx is None:
        return lines + ["", "## Log", "", entry]
    # Insert after the last non-blank line in the existing log block.
    insert_at = len(lines)
    for j in range(insert_at - 1, log_idx, -1):
        if lines[j].strip():
            insert_at = j + 1
            break
    lines.insert(insert_at, entry)
    return lines


# ---------------------------------------------------------------------------
# Text rendering
# ---------------------------------------------------------------------------


def render_text(env: Envelope) -> str:
    lines: list[str] = []
    lines.append(f"epic-children: {env.query['epic']} ({env.query['path']})")
    lines.append(env.narrative)
    drift_rows = [r for r in env.results if r["drift_kind"] != "consistent"]
    if drift_rows:
        lines.append("")
        lines.append("Drift rows:")
        for r in drift_rows:
            note = f" — {r['note']}" if r["note"] else ""
            lines.append(
                f"  {r['id']:>5}  {r['drift_kind']:<26}  "
                f"dashboard={r['dashboard_claim']!r:<14} "
                f"frontmatter={r['frontmatter_status']!r}{note}"
            )
        omitted = env.scan_stats["roster_entries"] - len(drift_rows)
        if omitted > 0:
            lines.append(f"  … {omitted} consistent rows omitted")
    if env.next:
        lines.append("")
        lines.append("Suggested next:")
        for n in env.next:
            lines.append(f"  - {n}")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def run(epic_arg: str, *, fix: bool, text: bool, quiet: bool) -> int:
    try:
        epic_path = resolve_epic_path(epic_arg)
    except SystemExit as exc:
        print(str(exc), file=sys.stderr)
        return 2

    try:
        original_text = epic_path.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"epic-children: cannot read {epic_path}: {exc}", file=sys.stderr)
        return 2

    epic_lines = original_text.splitlines()
    rows, header_idx, end_idx = parse_roster(epic_lines)
    if header_idx is None:
        print(
            f"epic-children: no roster table located in "
            f"{epic_path.relative_to(REPO_ROOT)}; expected '### Open child "
            f"tickets' with a 'Ticket | Status | ...' header",
            file=sys.stderr,
        )
        return 2

    roster_ids = {r.id for r in rows if r.id != "???"}
    span = (header_idx, end_idx) if end_idx else (header_idx, header_idx + 1)
    section_refs = collect_section_refs(epic_lines, roster_ids, span)

    index = build_ticket_index_with_landed(REPO_ROOT)
    drifts = [classify_row(r, index) for r in rows]

    # Derive an `epic_id` for the envelope. Prefer the leading-numeric
    # prefix of the filename; fall back to the literal arg.
    name_match = re.match(r"^(\d+[a-z]?)-", epic_path.name)
    epic_id = (
        _normalize_child_id(name_match.group(1)) if name_match else epic_arg
    )

    fixed_count = 0
    fixed_records: list[tuple[str, str]] = []
    if fix:
        today = dt.date.today().isoformat()
        fixed_count, fixed_records = apply_fix(
            epic_path, epic_lines, rows, drifts, index, today
        )
        # Re-classify after the rewrite to refresh the envelope's `results`.
        if fixed_count:
            new_text = epic_path.read_text(encoding="utf-8")
            new_lines = new_text.splitlines()
            rows, header_idx, end_idx = parse_roster(new_lines)
            drifts = [classify_row(r, index) for r in rows]

    env = render_envelope(
        epic_path, epic_id, epic_arg, rows, drifts, section_refs, fix, fixed_count
    )

    if quiet:
        # Print only the narrative + any failure line so `just check`
        # surfaces something readable; never JSON.
        if env.scan_stats["drift_detected"] > 0 and not fix:
            print(env.narrative)
    elif text:
        print(render_text(env))
    else:
        print(json.dumps(_asdict(env), indent=2, ensure_ascii=False))

    if fix:
        # After a fix run, exit code reflects post-fix state.
        return 0 if env.scan_stats["drift_detected"] == 0 else 1
    return 0 if env.scan_stats["drift_detected"] == 0 else 1


def _asdict(env: Envelope) -> dict[str, Any]:
    return {
        "query": env.query,
        "scan_stats": env.scan_stats,
        "results": env.results,
        "section_refs": env.section_refs,
        "narrative": env.narrative,
        "next": env.next,
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument(
        "epic",
        nargs="?",
        default="060",
        help="Epic id (e.g. 060) or path to the epic file (default: 060)",
    )
    ap.add_argument(
        "--fix",
        action="store_true",
        help="Rewrite the roster table to match frontmatter (status / blockers / "
        "landed sha). Skips missing-file and link-mismatch rows.",
    )
    ap.add_argument(
        "--text",
        action="store_true",
        help="Render a human-readable summary instead of the default JSON.",
    )
    ap.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress stdout; signal drift via exit code only. "
        "Used by `scripts/check_epic_children.sh`.",
    )
    args = ap.parse_args(argv)

    if args.text and args.quiet:
        print(
            "epic-children: --text and --quiet are mutually exclusive",
            file=sys.stderr,
        )
        return 2
    if args.fix and args.quiet:
        print(
            "epic-children: --fix and --quiet are mutually exclusive "
            "(fix should always log what it touched)",
            file=sys.stderr,
        )
        return 2

    return run(args.epic, fix=args.fix, text=args.text, quiet=args.quiet)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
