#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Land or backfill a Clowder open-work ticket.

Two modes:

  uv run scripts/land_ticket.py <id> [--log "<entry>"]
      Land a ticket: flip status -> done, set landed-at: pending,
      landed-on: <today>, append optional Log entry, move file from
      `docs/open-work/tickets/` to `docs/open-work/landed/`, drop the
      ticket id from any other ticket's blocked-by list, and regenerate
      `docs/open-work.md`.

  uv run scripts/land_ticket.py <id> --sha <short-sha>
      Backfill: rewrite landed-at: pending -> landed-at: <sha> in the
      already-landed file. Idempotent if landed-at already matches.

Per CLAUDE.md "Long-horizon coordination" + memory
`feedback_landing_unblock_routine.md`, both modes regenerate the index
in the same call so it doesn't drift.

Usage:
    just land <id> [--log "<entry>"]
    just land <id> --sha <short-sha>
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"
LANDED_DIR = REPO_ROOT / "docs" / "open-work" / "landed"
GENERATE_INDEX = REPO_ROOT / "scripts" / "generate_open_work.py"


def find_ticket_file(ticket_id: str, in_dir: Path) -> Path | None:
    """Return the unique `<id>-*.md` under `in_dir`, or None."""
    if not in_dir.exists():
        return None
    matches = sorted(in_dir.glob(f"{ticket_id}-*.md"))
    if not matches:
        return None
    if len(matches) > 1:
        raise SystemExit(
            f"land_ticket: multiple files match id={ticket_id} in {in_dir}: "
            + ", ".join(p.name for p in matches)
        )
    return matches[0]


def split_frontmatter(text: str) -> tuple[list[str], list[str]]:
    """Split a markdown file into (frontmatter_lines, body_lines).

    Frontmatter is the block between the first two `---` lines on their
    own. Returns ([], all_lines) if no frontmatter is found.
    """
    lines = text.splitlines(keepends=False)
    if not lines or lines[0].strip() != "---":
        return [], lines
    end = None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            end = i
            break
    if end is None:
        return [], lines
    return lines[1:end], lines[end + 1:]


def rewrite_frontmatter_field(fm_lines: list[str], key: str, value: str) -> list[str]:
    """Set `key: value` in frontmatter; replace existing line or append.

    Preserves order; the new line uses bare scalar form (no quoting),
    matching the existing file style.
    """
    pattern = re.compile(rf"^{re.escape(key)}\s*:\s*.*$")
    new_line = f"{key}: {value}"
    for i, line in enumerate(fm_lines):
        if pattern.match(line):
            fm_lines[i] = new_line
            return fm_lines
    fm_lines.append(new_line)
    return fm_lines


def assemble(fm_lines: list[str], body_lines: list[str]) -> str:
    return (
        "---\n"
        + "\n".join(fm_lines)
        + "\n---\n"
        + "\n".join(body_lines)
        + ("\n" if body_lines and not body_lines[-1].endswith("\n") else "")
    )


def append_log_entry(body_lines: list[str], entry: str, today: str) -> list[str]:
    """Append `- <today>: <entry>` under the existing `## Log` section.

    If no `## Log` heading exists, append one at the end.
    """
    log_idx = next(
        (i for i, line in enumerate(body_lines) if line.strip() == "## Log"),
        None,
    )
    line = f"- {today}: {entry}"
    if log_idx is None:
        body_lines = body_lines + ["", "## Log", "", line]
        return body_lines
    insert_at = len(body_lines)
    for j in range(insert_at - 1, log_idx, -1):
        if body_lines[j].strip():
            insert_at = j + 1
            break
    body_lines.insert(insert_at, line)
    return body_lines


def drop_blocked_by(ticket_id: str) -> list[Path]:
    """Drop `ticket_id` from every blocked-by list across tickets/ + landed/.

    Returns the paths that were modified.
    """
    modified: list[Path] = []
    target_int = int(ticket_id)
    for directory in (TICKETS_DIR, LANDED_DIR):
        if not directory.exists():
            continue
        for path in directory.glob("*.md"):
            if path.name.startswith("_"):
                continue
            text = path.read_text(encoding="utf-8")
            fm_lines, body_lines = split_frontmatter(text)
            if not fm_lines:
                continue
            new_fm, changed = _strip_id_from_blocked_by(fm_lines, target_int)
            if changed:
                path.write_text(assemble(new_fm, body_lines), encoding="utf-8")
                modified.append(path)
    return modified


def _strip_id_from_blocked_by(fm_lines: list[str], target_int: int) -> tuple[list[str], bool]:
    """Drop `target_int` from a `blocked-by:` field; flow- or block-style.

    Returns (new_lines, changed).
    """
    pattern = re.compile(r"^blocked-by\s*:\s*(.*)$")
    flow_pattern = re.compile(r"\[(.*)\]")
    new_lines = list(fm_lines)
    changed = False
    for i, line in enumerate(new_lines):
        m = pattern.match(line)
        if not m:
            continue
        rest = m.group(1).strip()
        flow = flow_pattern.match(rest)
        if flow is not None:
            inner = flow.group(1).strip()
            if not inner:
                return new_lines, False
            ids = [x.strip() for x in inner.split(",") if x.strip()]
            kept = [x for x in ids if _as_int(x) != target_int]
            if len(kept) != len(ids):
                new_lines[i] = "blocked-by: [" + ", ".join(kept) + "]"
                changed = True
            return new_lines, changed
        if rest == "" or rest == "null":
            return new_lines, False
        j = i + 1
        list_items: list[tuple[int, str]] = []
        while j < len(new_lines):
            stripped = new_lines[j].lstrip()
            if not stripped.startswith("- "):
                break
            list_items.append((j, stripped[2:].strip()))
            j += 1
        if not list_items:
            return new_lines, False
        keep_idxs = [
            j for (j, value) in list_items if _as_int(value) != target_int
        ]
        if len(keep_idxs) == len(list_items):
            return new_lines, False
        drop_idxs = sorted(
            [j for (j, value) in list_items if _as_int(value) == target_int],
            reverse=True,
        )
        for di in drop_idxs:
            del new_lines[di]
        if not keep_idxs:
            new_lines[i] = "blocked-by: []"
        return new_lines, True
    return new_lines, False


def _as_int(token: str) -> int | None:
    try:
        return int(token.strip())
    except ValueError:
        return None


def regenerate_index() -> None:
    subprocess.run(
        ["uv", "run", str(GENERATE_INDEX)],
        cwd=REPO_ROOT,
        check=True,
    )


def cmd_land(ticket_id: str, log_entry: str | None) -> int:
    src = find_ticket_file(ticket_id, TICKETS_DIR)
    if src is None:
        existing = find_ticket_file(ticket_id, LANDED_DIR)
        if existing is not None:
            print(
                f"land_ticket: ticket {ticket_id} is already landed at {existing}. "
                f"Use `--sha` to backfill if landed-at is still pending.",
                file=sys.stderr,
            )
            return 1
        print(
            f"land_ticket: no ticket file found for id={ticket_id} in {TICKETS_DIR}",
            file=sys.stderr,
        )
        return 1

    today = dt.date.today().isoformat()
    text = src.read_text(encoding="utf-8")
    fm_lines, body_lines = split_frontmatter(text)
    if not fm_lines:
        print(f"land_ticket: {src} has no frontmatter", file=sys.stderr)
        return 1
    fm_lines = rewrite_frontmatter_field(fm_lines, "status", "done")
    fm_lines = rewrite_frontmatter_field(fm_lines, "landed-at", "pending")
    fm_lines = rewrite_frontmatter_field(fm_lines, "landed-on", today)
    if log_entry:
        body_lines = append_log_entry(body_lines, log_entry, today)
    src.write_text(assemble(fm_lines, body_lines), encoding="utf-8")

    LANDED_DIR.mkdir(parents=True, exist_ok=True)
    dst = LANDED_DIR / src.name
    if dst.exists():
        print(
            f"land_ticket: refusing to overwrite existing {dst}",
            file=sys.stderr,
        )
        return 1
    src.rename(dst)

    unblocked = drop_blocked_by(ticket_id)
    regenerate_index()

    print(f"landed: {dst.relative_to(REPO_ROOT)}")
    if unblocked:
        print(f"  unblocked from {len(unblocked)} dependent(s):")
        for path in unblocked:
            print(f"    - {path.relative_to(REPO_ROOT)}")
    print(f"  landed-at: pending — run `just land {ticket_id} --sha <sha>` after committing")
    return 0


def cmd_backfill(ticket_id: str, sha: str) -> int:
    target = find_ticket_file(ticket_id, LANDED_DIR)
    if target is None:
        in_tickets = find_ticket_file(ticket_id, TICKETS_DIR)
        if in_tickets is not None:
            print(
                f"land_ticket: ticket {ticket_id} is still in tickets/. "
                f"Run `just land {ticket_id}` first, then backfill with --sha.",
                file=sys.stderr,
            )
            return 1
        print(
            f"land_ticket: no landed file found for id={ticket_id} in {LANDED_DIR}",
            file=sys.stderr,
        )
        return 1

    text = target.read_text(encoding="utf-8")
    fm_lines, body_lines = split_frontmatter(text)
    if not fm_lines:
        print(f"land_ticket: {target} has no frontmatter", file=sys.stderr)
        return 1

    current = next(
        (l for l in fm_lines if l.startswith("landed-at:")),
        None,
    )
    if current and current.split(":", 1)[1].strip() == sha:
        print(f"land_ticket: landed-at already {sha} (idempotent no-op)")
        return 0

    fm_lines = rewrite_frontmatter_field(fm_lines, "landed-at", sha)
    target.write_text(assemble(fm_lines, body_lines), encoding="utf-8")

    regenerate_index()
    print(f"backfilled: {target.relative_to(REPO_ROOT)} landed-at={sha}")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("ticket_id", help="Ticket id (e.g. 197 — leading zeros optional)")
    ap.add_argument(
        "--log", default=None,
        help="Append `- <today>: <log>` under the ticket's `## Log` section",
    )
    ap.add_argument(
        "--sha", default=None,
        help="Backfill landed-at with the given short sha (skip the move)",
    )
    args = ap.parse_args(argv)

    ticket_id = args.ticket_id.lstrip("0") or "0"

    if args.sha is not None:
        if args.log is not None:
            print(
                "land_ticket: --log is for the initial land step, not backfill",
                file=sys.stderr,
            )
            return 2
        return cmd_backfill(ticket_id, args.sha)

    return cmd_land(ticket_id, args.log)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
