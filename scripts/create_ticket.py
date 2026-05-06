#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Open a new Clowder open-work ticket from a template.

Picks the next available ticket id by walking `docs/open-work/tickets/`
and `docs/open-work/landed/`, instantiates the chosen template
(`_template.md` by default; `_template_bugfix.md` when `--bugfix`),
fills in `id` / `title` / `added` / `cluster` / `blocked-by`, and
regenerates `docs/open-work.md`.

Usage:
    just open-ticket "<title>" [options]

Options:
    --bugfix                   Use _template_bugfix.md (audit table + structural-option slot)
    --cluster <name>           Set frontmatter `cluster:` (e.g. process-discipline)
    --blocked-by <ids>         Comma-separated ticket ids; sets `status: blocked` automatically
    --slug <slug>              Override slug derived from title
    --id <id>                  Pin a specific id instead of next-available

Mirror of `scripts/land_ticket.py` — both use the same frontmatter
helpers from `generate_open_work.py` so the source-of-truth shape stays
consistent.
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
TEMPLATE_DEFAULT = TICKETS_DIR / "_template.md"
TEMPLATE_BUGFIX = TICKETS_DIR / "_template_bugfix.md"


def all_ticket_ids() -> list[int]:
    """Every 3-digit ticket id reachable in tickets/ + landed/ (deduped).

    Restricted to exactly 3 digits to skip the date-prefixed legacy files
    in `landed/` (e.g. `2026-04-19-v0-2-0-release-...md`) that predate
    the numbered convention.
    """
    ids: set[int] = set()
    for directory in (TICKETS_DIR, LANDED_DIR):
        if not directory.exists():
            continue
        for path in directory.glob("*.md"):
            if path.name.startswith("_"):
                continue
            m = re.match(r"^(\d{3})-", path.name)
            if m:
                ids.add(int(m.group(1)))
    return sorted(ids)


def next_id() -> int:
    ids = all_ticket_ids()
    return (ids[-1] + 1) if ids else 1


def slugify(title: str) -> str:
    """Lowercase ASCII-ish slug. Mirrors how existing ticket files are named."""
    s = title.lower().strip()
    s = re.sub(r"[^\w\s-]", "", s)
    s = re.sub(r"[\s_]+", "-", s)
    s = re.sub(r"-+", "-", s).strip("-")
    return s or "untitled"


def render_ticket(template_path: Path, *,
                  ticket_id: int, title: str,
                  cluster: str | None,
                  blocked_by: list[int],
                  today: str) -> str:
    text = template_path.read_text(encoding="utf-8")

    def sub_frontmatter(line: str) -> str:
        # Drop trailing comments after `# ` so the rendered file is clean.
        comment_idx = line.find("  #")
        if comment_idx == -1:
            comment_idx = line.find("\t#")
        body = line[:comment_idx] if comment_idx != -1 else line
        body = body.rstrip()
        if body.startswith("id:"):
            return f"id: {ticket_id}"
        if body.startswith("title:"):
            return f"title: {title}"
        if body.startswith("status:"):
            status = "blocked" if blocked_by else "ready"
            return f"status: {status}"
        if body.startswith("cluster:"):
            return f"cluster: {cluster if cluster else 'null'}"
        if body.startswith("added:"):
            return f"added: {today}"
        if body.startswith("parked:"):
            return "parked: null"
        if body.startswith("blocked-by:"):
            if not blocked_by:
                return "blocked-by: []"
            return "blocked-by: [" + ", ".join(str(b) for b in blocked_by) + "]"
        if body.startswith("supersedes:"):
            return "supersedes: []"
        if body.startswith("related-systems:"):
            return "related-systems: []"
        if body.startswith("related-balance:"):
            return "related-balance: []"
        if body.startswith("landed-at:"):
            return "landed-at: null"
        if body.startswith("landed-on:"):
            return "landed-on: null"
        return body

    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        raise SystemExit(f"create_ticket: template {template_path} has no frontmatter")
    end = None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            end = i
            break
    if end is None:
        raise SystemExit(f"create_ticket: template {template_path} frontmatter unterminated")

    rendered_fm = [sub_frontmatter(line) for line in lines[1:end]]
    body = "\n".join(lines[end + 1:])
    return "---\n" + "\n".join(rendered_fm) + "\n---\n" + body + ("\n" if not body.endswith("\n") else "")


def regenerate_index() -> None:
    subprocess.run(
        ["uv", "run", str(GENERATE_INDEX)],
        cwd=REPO_ROOT,
        check=True,
    )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("title", help="Ticket title (one short sentence)")
    ap.add_argument("--bugfix", action="store_true",
                    help="Use _template_bugfix.md (audit table + structural-option slot)")
    ap.add_argument("--cluster", default=None,
                    help="Set frontmatter `cluster:` (e.g. process-discipline)")
    ap.add_argument("--blocked-by", default="",
                    help="Comma-separated ticket ids; sets status: blocked automatically")
    ap.add_argument("--slug", default=None,
                    help="Override slug derived from title")
    ap.add_argument("--id", dest="forced_id", type=int, default=None,
                    help="Pin a specific id instead of next-available")
    args = ap.parse_args(argv)

    today = dt.date.today().isoformat()
    template = TEMPLATE_BUGFIX if args.bugfix else TEMPLATE_DEFAULT
    if not template.exists():
        print(f"create_ticket: template not found at {template}", file=sys.stderr)
        return 1

    ticket_id = args.forced_id if args.forced_id is not None else next_id()
    if args.forced_id is not None and ticket_id in all_ticket_ids():
        print(f"create_ticket: id {ticket_id} is already in use", file=sys.stderr)
        return 1

    blocked_by_ids: list[int] = []
    for token in args.blocked_by.split(","):
        token = token.strip()
        if not token:
            continue
        try:
            blocked_by_ids.append(int(token))
        except ValueError:
            print(f"create_ticket: --blocked-by token '{token}' is not an int", file=sys.stderr)
            return 1

    slug = args.slug or slugify(args.title)
    out_path = TICKETS_DIR / f"{ticket_id:03d}-{slug}.md"
    if out_path.exists():
        print(f"create_ticket: refusing to overwrite existing {out_path}", file=sys.stderr)
        return 1

    rendered = render_ticket(
        template,
        ticket_id=ticket_id,
        title=args.title,
        cluster=args.cluster,
        blocked_by=blocked_by_ids,
        today=today,
    )
    out_path.write_text(rendered, encoding="utf-8")

    regenerate_index()

    print(f"created: {out_path.relative_to(REPO_ROOT)}")
    if args.bugfix:
        print("  template: _template_bugfix.md (fill the layer-walk audit table)")
    if blocked_by_ids:
        print(f"  status: blocked  (blocked-by: {blocked_by_ids})")
    print(f"  next: open in your editor and fill out ## Why")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
