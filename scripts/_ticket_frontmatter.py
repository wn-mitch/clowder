#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Shared frontmatter + ticket-index helpers for Clowder open-work tooling.

Lifted from `generate_open_work.py` so that `epic_children.py` (and any
future open-work query tool) reuses the same minimal-YAML parser, the
same `Ticket` shape, and the same `id → Ticket` index builder. Two
variants of the index are exposed:

- `build_ticket_index(repo_root)` — back-compat for `generate_open_work.py`;
  active tickets shadow landed ones (first-write-wins on collision).
- `build_ticket_index_with_landed(repo_root)` — returns both copies as
  `(active, landed)` so callers (epic-children) can detect the
  landed-but-marked-active drift kind.

No external dependencies. Pure stdlib so the PEP 723 `dependencies = []`
header in every consumer stays trivially satisfiable.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
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
    def initiative(self) -> list:
        val = self.frontmatter.get("initiative") or []
        return val if isinstance(val, list) else [val]

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

    @property
    def orchestration(self) -> str:
        """The three-track orchestration axis (ticket 354).

        Returns one of substrate-sensitive / coherent-block / swarm-safe,
        or empty string if untagged (which `just check` rejects).
        """
        return str(self.frontmatter.get("orchestration") or "")

    @property
    def block(self) -> str:
        """Coherent-block name; only meaningful when orchestration is
        coherent-block (enforced by check_orchestration_frontmatter.py)."""
        return str(self.frontmatter.get("block") or "")

    @property
    def verdict_anchor(self) -> bool:
        """True iff this ticket is the verdict-anchor for its block."""
        return self.frontmatter.get("verdict-anchor") in (True, "true")


def load_tickets(tickets_dir: Path) -> list[Ticket]:
    tickets = []
    if not tickets_dir.exists():
        return tickets
    for p in sorted(tickets_dir.glob("*.md")):
        if p.name.startswith("_") or p.name.lower() == "readme.md":
            continue
        text = p.read_text(encoding="utf-8")
        fm = parse_frontmatter(text)
        # Body kept empty for the back-compat index path (matches the
        # legacy `load_tickets` shape in generate_open_work.py). Callers
        # that need the body should read the file directly.
        tickets.append(Ticket(path=p, frontmatter=fm, body=""))
    return tickets


# ---------------------------------------------------------------------------
# Id normalization + child-link regex (shared with epic-progress logic)
# ---------------------------------------------------------------------------


def _format_id(raw) -> str:
    """Zero-pad to three digits when the input is a small integer-ish id."""
    if isinstance(raw, int):
        return f"{raw:03d}"
    try:
        return f"{int(str(raw)):03d}"
    except (TypeError, ValueError):
        return str(raw)


def _normalize_child_id(raw: str) -> str:
    """Pad numeric prefix to 3 digits, preserving any letter suffix
    (`27b` -> `027b`)."""
    m = re.match(r"^(\d+)([a-z]?)$", raw)
    if not m:
        return raw
    num, suffix = m.group(1), m.group(2)
    return f"{int(num):03d}{suffix}"


# Match a markdown link target whose filename starts with NNN- (1-3 digits +
# optional single letter) and ends in .md. Anchored to `(` or `/` so we
# never grab the year prefix on date-named landed files (`2026-04-19-...`).
_CHILD_LINK_RE = re.compile(
    r"(?:\(|/)(\d{1,3}[a-z]?)-[A-Za-z0-9_-]+\.md(?=\))"
)


# ---------------------------------------------------------------------------
# Index builders
# ---------------------------------------------------------------------------


_TICKET_SUBDIRS = ("tickets", "landed", "pre-existing")


def build_ticket_index(repo_root: Path) -> dict[str, Ticket]:
    """Merge tickets/ + landed/ + pre-existing/ into a single id → Ticket map.

    First-write-wins: open tickets shadow landed if both exist. In
    practice `land_ticket.py` moves files between directories so a
    duplicate shouldn't happen — but if one does, the active copy
    wins (the iteration order `tickets/` → `landed/` → `pre-existing/`
    enforces this).
    """
    out: dict[str, Ticket] = {}
    for sub in _TICKET_SUBDIRS:
        d = repo_root / "docs" / "open-work" / sub
        for t in load_tickets(d):
            out.setdefault(t.id, t)
    return out


def build_ticket_index_with_landed(
    repo_root: Path,
) -> dict[str, tuple[Ticket | None, Ticket | None]]:
    """Return id → (active_or_pre_existing, landed) per ticket id.

    Unlike `build_ticket_index`, this keeps **both** copies when a
    ticket appears in `tickets/` (or `pre-existing/`) AND in `landed/`.
    That's the dual-file case epic-children needs to detect for the
    `landed-but-marked-active` drift kind.

    "Active" here means any non-landed origin: `tickets/` or
    `pre-existing/`. `pre-existing/` files don't usually carry a
    landed copy, but if they did, they'd populate the first slot.
    """
    docs_root = repo_root / "docs" / "open-work"
    active_dirs = ("tickets", "pre-existing")
    landed_dir = "landed"

    active_by_id: dict[str, Ticket] = {}
    for sub in active_dirs:
        for t in load_tickets(docs_root / sub):
            active_by_id.setdefault(t.id, t)

    landed_by_id: dict[str, Ticket] = {}
    for t in load_tickets(docs_root / landed_dir):
        landed_by_id.setdefault(t.id, t)

    out: dict[str, tuple[Ticket | None, Ticket | None]] = {}
    for tid in set(active_by_id) | set(landed_by_id):
        out[tid] = (active_by_id.get(tid), landed_by_id.get(tid))
    return out
