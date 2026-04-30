#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
One-shot migration: split docs/open-work/landed/YYYY-MM.md monthly archives
into per-file landed tickets (docs/open-work/landed/<id>-<slug>.md), with
frontmatter that mirrors the active tickets/ layout.

Each `## ` H2 in the source file with a date in the heading becomes one
output file. Three heading shapes are recognized:

    ## Ticket NNN — Title (YYYY-MM-DD)
    ## Tickets NNN + MMM [+ ...] — Title (YYYY-MM-DD)
    ## Some title (YYYY-MM-DD[, ticket NNN][, commit `sha`])

The first numeric ticket id wins (heading prefix beats parenthetical).
Entries without any ticket id get a date-derived slug-id (no leading number)
so they remain discoverable but don't collide with numeric tickets.

Run idempotently:
    uv run scripts/migrate_landed_archive.py --dry-run
    uv run scripts/migrate_landed_archive.py
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


HEADING_RE = re.compile(r"^## (.+\(\d{4}-\d{2}-\d{2}.*\).*)$", re.MULTILINE)
DATE_RE = re.compile(r"\((\d{4}-\d{2}-\d{2})")
TICKET_PREFIX_RE = re.compile(r"^Ticket\s+(\d+)\b")
TICKETS_PREFIX_RE = re.compile(r"^Tickets\s+(\d+(?:\s*\+\s*\d+)+)")
PE_PREFIX_RE = re.compile(r"^(PE-\d+)\b")
TICKET_PAREN_RE = re.compile(r"ticket\s+(\d+)", re.IGNORECASE)
TICKETS_PAREN_RE = re.compile(r"tickets\s+(\d+)\s*\+\s*(\d+)", re.IGNORECASE)
SHA_RE = re.compile(r"`?([0-9a-f]{7,40})`?")


@dataclass
class Entry:
    raw_heading: str
    title: str
    landed_on: str | None
    primary_id: str
    extra_ids: list[str]
    landed_at: str | None
    body: str

    @property
    def filename(self) -> str:
        return f"{self.primary_id}-{slugify(self.title)}.md"


def slugify(text: str, max_len: int = 60) -> str:
    text = text.lower()
    text = re.sub(r"[^a-z0-9]+", "-", text)
    text = re.sub(r"-+", "-", text).strip("-")
    if len(text) > max_len:
        text = text[:max_len].rstrip("-")
    return text or "untitled"


def parse_heading(heading: str) -> tuple[str, str | None, str, list[str]]:
    """Return (title, landed_on, primary_id, extra_ids)."""
    date_match = DATE_RE.search(heading)
    landed_on = date_match.group(1) if date_match else None

    title = heading
    if date_match:
        title = heading[: date_match.start()].rstrip().rstrip(",").rstrip()
    title = title.strip()

    primary_id: str | None = None
    extra_ids: list[str] = []

    if (m := TICKETS_PREFIX_RE.match(title)):
        nums = [int(x) for x in re.findall(r"\d+", m.group(1))]
        primary_id = f"{nums[0]:03d}"
        extra_ids.extend(f"{n:03d}" for n in nums[1:])
        title = title[m.end():].lstrip(" —-").strip()
    elif (m := TICKET_PREFIX_RE.match(title)):
        primary_id = f"{int(m.group(1)):03d}"
        title = title[m.end():].lstrip(" —-").strip()
    elif (m := PE_PREFIX_RE.match(title)):
        primary_id = m.group(1)
        title = title[m.end():].lstrip(" —-").strip()

    if primary_id is None and date_match:
        paren_text = heading[date_match.start():]
        if (m := TICKETS_PAREN_RE.search(paren_text)):
            primary_id = f"{int(m.group(1)):03d}"
            extra_ids.append(f"{int(m.group(2)):03d}")
        elif (m := TICKET_PAREN_RE.search(paren_text)):
            primary_id = f"{int(m.group(1)):03d}"

    if primary_id is None:
        primary_id = landed_on or "undated"

    return title, landed_on, primary_id, extra_ids


def extract_landed_at(body: str) -> str | None:
    """First sha-shaped token in any line that names a commit."""
    for line in body.splitlines():
        if "**Landed at:**" in line or "**Commit:**" in line or "**Commits" in line:
            if (m := SHA_RE.search(line.split(":", 1)[-1])):
                sha = m.group(1)
                return sha[:8] if len(sha) > 8 else sha
    if (m := SHA_RE.search(body)):
        sha = m.group(1)
        return sha[:8] if len(sha) > 8 else sha
    return None


def parse_entries(text: str) -> list[Entry]:
    entries: list[Entry] = []
    matches = list(HEADING_RE.finditer(text))
    for i, m in enumerate(matches):
        heading = m.group(1).strip()
        body_start = m.end() + 1
        body_end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        body = text[body_start:body_end].strip("\n")
        title, landed_on, primary_id, extra_ids = parse_heading(heading)
        landed_at = extract_landed_at(body)
        entries.append(
            Entry(
                raw_heading=heading,
                title=title,
                landed_on=landed_on,
                primary_id=primary_id,
                extra_ids=extra_ids,
                landed_at=landed_at,
                body=body,
            )
        )
    return entries


def yaml_string(text: str) -> str:
    needs_quote = any(c in text for c in ":#[]{}&*!|>'\"%@`,") or text.startswith("-")
    if needs_quote:
        escaped = text.replace('"', '\\"')
        return f'"{escaped}"'
    return text


def render_file(entry: Entry) -> str:
    fm: list[str] = ["---"]
    if re.fullmatch(r"\d+", entry.primary_id):
        fm.append(f"id: {int(entry.primary_id):03d}")
    else:
        fm.append(f"id: {entry.primary_id}")
    fm.append(f"title: {yaml_string(entry.title)}")
    fm.append("status: done")
    fm.append("cluster: null")
    if entry.extra_ids:
        ids = ", ".join(str(int(x)) if x.isdigit() else x for x in entry.extra_ids)
        fm.append(f"also-landed: [{ids}]")
    fm.append(f"landed-at: {entry.landed_at if entry.landed_at else 'null'}")
    fm.append(f"landed-on: {entry.landed_on if entry.landed_on else 'null'}")
    fm.append("---")
    fm.append("")
    fm.append(f"# {entry.title}")
    fm.append("")
    fm.append(entry.body.rstrip() + "\n")
    return "\n".join(fm)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be written without modifying the filesystem.",
    )
    args = parser.parse_args()

    repo_root = args.repo.resolve()
    landed_dir = repo_root / "docs" / "open-work" / "landed"
    if not landed_dir.exists():
        print(f"No landed dir at {landed_dir}", file=sys.stderr)
        return 1

    sources = [
        p for p in sorted(landed_dir.glob("*.md")) if p.name.lower() != "readme.md"
    ]
    if not sources:
        print("No monthly aggregate files to migrate.")
        return 0

    all_entries: list[tuple[Path, Entry]] = []
    for src in sources:
        text = src.read_text(encoding="utf-8")
        entries = parse_entries(text)
        for e in entries:
            all_entries.append((src, e))

    seen_filenames: dict[str, Entry] = {}
    collisions: list[tuple[str, Entry]] = []
    for _, e in all_entries:
        fn = e.filename
        if fn in seen_filenames:
            new_fn = f"{e.primary_id}-{slugify(e.title)}-{slugify(e.landed_on or 'x')}.md"
            collisions.append((fn, e))
            seen_filenames[new_fn] = e
        else:
            seen_filenames[fn] = e

    print(f"Found {len(all_entries)} entries across {len(sources)} archive file(s).")
    if collisions:
        print(f"  {len(collisions)} filename collision(s) resolved with date suffix.")

    by_kind: dict[str, int] = {}
    for _, e in all_entries:
        kind = "numeric" if e.primary_id.isdigit() else "non-numeric"
        by_kind[kind] = by_kind.get(kind, 0) + 1
    print(f"  numeric ids:     {by_kind.get('numeric', 0)}")
    print(f"  non-numeric ids: {by_kind.get('non-numeric', 0)}")

    written = 0
    skipped = 0
    for filename, entry in seen_filenames.items():
        out_path = landed_dir / filename
        if out_path.exists():
            skipped += 1
            continue
        rendered = render_file(entry)
        if args.dry_run:
            print(f"  [dry-run] would write {out_path.relative_to(repo_root)}")
        else:
            out_path.write_text(rendered, encoding="utf-8")
        written += 1

    print(f"  written: {written}  skipped (already exists): {skipped}")

    if args.dry_run:
        print("Dry-run complete. No files modified.")
        return 0

    if written != len(all_entries) - skipped:
        print(
            f"Aborting source deletion: wrote {written} files but expected "
            f"{len(all_entries) - skipped}.",
            file=sys.stderr,
        )
        return 2

    for src in sources:
        src.unlink()
        print(f"  removed {src.relative_to(repo_root)}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
