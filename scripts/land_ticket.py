#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Land or backfill a Clowder open-work ticket.

Three modes:

  uv run scripts/land_ticket.py <id> [--log "<entry>"]
      File-only land: flip status -> done, set landed-at: pending,
      landed-on: <today>, append optional Log entry, move file from
      `docs/open-work/tickets/` to `docs/open-work/landed/`, drop the
      ticket id from every dependent's blocked-by list, and regenerate
      `docs/open-work.md`. Does NOT touch jj — the user commits and
      backfills the sha themselves.

  uv run scripts/land_ticket.py <id> --sha <short-sha>
      Backfill: rewrite landed-at: pending -> landed-at: <sha> in the
      already-landed file. Idempotent if landed-at already matches.

  uv run scripts/land_ticket.py <id> --commit "<feat-message>" [--log "<entry>"]
      Full jj-orchestrated land. Treats the current working copy (@) as
      the implementation, applies the file-mode landing on top, runs
      `jj describe -m "<feat-message>"`, reads the new sha,
      `jj new -m '(empty)'`, applies sha backfill, describes the
      backfill commit, then runs one more `jj new` so @ is fresh for
      the next ticket. End state: 2 stable commits + empty @.

Per CLAUDE.md "Long-horizon coordination" + memory
`feedback_landing_unblock_routine.md`, every mode regenerates the
index so it doesn't drift.

Usage:
    just land <id> [--log "<entry>"]
    just land <id> --sha <short-sha>
    just land <id> --commit "feat: <id> — <summary>" [--log "<entry>"]
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


def jj(*args: str, capture: bool = False) -> str:
    """Run a `jj` subcommand. Returns stdout (stripped) when capture=True."""
    proc = subprocess.run(
        ["jj", *args],
        cwd=REPO_ROOT,
        check=True,
        capture_output=capture,
        text=True,
    )
    return proc.stdout.strip() if capture else ""


def jj_head_sha(short: bool = True) -> str:
    """Return @-'s commit sha (short by default).

    `@-` is the parent of the current working copy — i.e. the most
    recent committed change. Used to read the sha of a commit we just
    described so we can backfill landed-at.
    """
    template = "commit_id.short()" if short else "commit_id"
    return jj("log", "-r", "@-", "--no-graph", "-T", template + r' ++ "\n"', capture=True).strip()


def apply_landing(ticket_id: str, log_entry: str | None) -> tuple[Path, list[Path]]:
    """File-only landing: rewrite frontmatter, move ticket, drop blocked-by.

    Returns (landed_path, unblocked_paths). Caller is responsible for
    regenerating the index — this function leaves it alone so callers
    that orchestrate further changes (the --commit path) can regen
    once at the end.
    """
    src = find_ticket_file(ticket_id, TICKETS_DIR)
    if src is None:
        existing = find_ticket_file(ticket_id, LANDED_DIR)
        if existing is not None:
            raise SystemExit(
                f"land_ticket: ticket {ticket_id} is already landed at {existing}. "
                f"Use `--sha` to backfill if landed-at is still pending."
            )
        raise SystemExit(
            f"land_ticket: no ticket file found for id={ticket_id} in {TICKETS_DIR}"
        )

    today = dt.date.today().isoformat()
    text = src.read_text(encoding="utf-8")
    fm_lines, body_lines = split_frontmatter(text)
    if not fm_lines:
        raise SystemExit(f"land_ticket: {src} has no frontmatter")
    fm_lines = rewrite_frontmatter_field(fm_lines, "status", "done")
    fm_lines = rewrite_frontmatter_field(fm_lines, "landed-at", "pending")
    fm_lines = rewrite_frontmatter_field(fm_lines, "landed-on", today)
    if log_entry:
        body_lines = append_log_entry(body_lines, log_entry, today)
    src.write_text(assemble(fm_lines, body_lines), encoding="utf-8")

    LANDED_DIR.mkdir(parents=True, exist_ok=True)
    dst = LANDED_DIR / src.name
    if dst.exists():
        raise SystemExit(f"land_ticket: refusing to overwrite existing {dst}")
    src.rename(dst)

    unblocked = drop_blocked_by(ticket_id)
    return (dst, unblocked)


def apply_sha_backfill(ticket_id: str, sha: str) -> Path:
    """Rewrite landed-at: pending -> landed-at: <sha> in the landed file."""
    target = find_ticket_file(ticket_id, LANDED_DIR)
    if target is None:
        raise SystemExit(
            f"land_ticket: no landed file found for id={ticket_id} in {LANDED_DIR}"
        )
    text = target.read_text(encoding="utf-8")
    fm_lines, body_lines = split_frontmatter(text)
    if not fm_lines:
        raise SystemExit(f"land_ticket: {target} has no frontmatter")
    fm_lines = rewrite_frontmatter_field(fm_lines, "landed-at", sha)
    target.write_text(assemble(fm_lines, body_lines), encoding="utf-8")
    return target


def cmd_land(ticket_id: str, log_entry: str | None) -> int:
    try:
        dst, unblocked = apply_landing(ticket_id, log_entry)
    except SystemExit as exc:
        print(str(exc), file=sys.stderr)
        return 1

    regenerate_index()

    print(f"landed: {dst.relative_to(REPO_ROOT)}")
    if unblocked:
        print(f"  unblocked from {len(unblocked)} dependent(s):")
        for path in unblocked:
            print(f"    - {path.relative_to(REPO_ROOT)}")
    print(f"  landed-at: pending — run `just land {ticket_id} --sha <sha>` after committing")
    return 0


def cmd_commit_land(ticket_id: str, message: str, log_entry: str | None) -> int:
    """Full jj-orchestrated landing.

    Sequence:
      1. Apply file-mode landing on top of current @ (which may already
         carry the implementation diff).
      2. Regenerate the index.
      3. `jj describe -m "<message>"` to name @.
      4. Read the new commit's sha.
      5. `jj new -m '(empty)'` so @ is fresh.
      6. Apply sha backfill in @.
      7. `jj describe -m "docs: backfill <id> landed-at sha to <sha>"`.
      8. `jj new -m '(empty)'` so the user has a clean working copy
         for the next task.

    End state: two committed revisions (feat + docs) + empty @.
    """
    try:
        dst, unblocked = apply_landing(ticket_id, log_entry)
    except SystemExit as exc:
        print(str(exc), file=sys.stderr)
        return 1

    regenerate_index()

    jj("describe", "-m", message)
    sha = jj_head_sha_from_at()
    jj("new", "-m", "(empty)")

    apply_sha_backfill(ticket_id, sha)
    regenerate_index()

    backfill_msg = f"docs: backfill {ticket_id} landed-at sha to {sha}"
    jj("describe", "-m", backfill_msg)
    jj("new", "-m", "(empty)")

    print(f"landed: {dst.relative_to(REPO_ROOT)}")
    print(f"  feat sha: {sha}")
    print(f"  feat msg: {message}")
    print(f"  docs msg: {backfill_msg}")
    if unblocked:
        print(f"  unblocked from {len(unblocked)} dependent(s):")
        for path in unblocked:
            print(f"    - {path.relative_to(REPO_ROOT)}")
    print("  working copy is empty — ready for the next ticket")
    return 0


def jj_head_sha_from_at() -> str:
    """Return @'s commit sha (short). Used after `jj describe` to read
    the sha of the just-named commit before we move past it with
    `jj new`."""
    return jj("log", "-r", "@", "--no-graph", "-T", r'commit_id.short() ++ "\n"',
              capture=True).strip()


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
    fm_lines, _ = split_frontmatter(text)
    current = next(
        (l for l in fm_lines if l.startswith("landed-at:")),
        None,
    )
    if current and current.split(":", 1)[1].strip() == sha:
        print(f"land_ticket: landed-at already {sha} (idempotent no-op)")
        return 0

    apply_sha_backfill(ticket_id, sha)
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
    ap.add_argument(
        "--commit", default=None, metavar="MESSAGE",
        help="Full jj-orchestrated land: bundle the current working copy "
             "with the landing diff, describe it with MESSAGE, then create "
             "a sha-backfill commit. End state: 2 committed revisions + "
             "empty @. Saves ~7 jj commands per landing.",
    )
    args = ap.parse_args(argv)

    ticket_id = args.ticket_id.lstrip("0") or "0"

    if args.sha is not None and args.commit is not None:
        print("land_ticket: --sha and --commit are mutually exclusive",
              file=sys.stderr)
        return 2

    if args.sha is not None:
        if args.log is not None:
            print(
                "land_ticket: --log is for the initial land step, not backfill",
                file=sys.stderr,
            )
            return 2
        return cmd_backfill(ticket_id, args.sha)

    if args.commit is not None:
        return cmd_commit_land(ticket_id, args.commit, args.log)

    return cmd_land(ticket_id, args.log)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
