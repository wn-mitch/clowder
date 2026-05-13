#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "pyyaml>=6.0",
# ]
# ///
"""
Apply approved cluster/initiative tag proposals to ticket frontmatter.

Reads a proposals YAML written by `suggest_tags.py` (or a hand-written file
in the same shape) and rewrites each approved ticket's frontmatter in place.

Only rewrites entries with `approve: true`. Untouched fields are preserved
byte-for-byte; only `cluster:` and/or `initiative:` lines change.

Usage:
    uv run scripts/apply_tags.py logs/tag-proposals/active.yaml
    uv run scripts/apply_tags.py logs/tag-proposals/seeds.yaml --dry-run

The proposals YAML shape (minimal — extra fields ignored):

    proposals:
      - ticket_id: "032"
        path: docs/open-work/tickets/032-starvation.md
        proposed_cluster: life-cycle
        proposed_initiatives: [welfare-fidelity]
        approve: true

After applying, the embedding index becomes stale; `just similar`,
`just next`, and the next `just open-work-index` invocation pick up the
changes via mtime invalidation.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent
TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"
LANDED_DIR = REPO_ROOT / "docs" / "open-work" / "landed"


CLUSTER_LINE_RE = re.compile(r"^(cluster:\s*)(\S.*?)(\s*(?:#.*)?)$")
INITIATIVE_LINE_RE = re.compile(r"^(initiative:\s*)(\S.*?)(\s*(?:#.*)?)$")


def resolve_path_by_id(ticket_id: str) -> Path | None:
    """Find the ticket file whose filename starts with `<id>-`, searching
    active tickets first, then landed. Hand-written YAML proposals often
    have stale or guessed paths; resolving by id is robust."""
    tid = ticket_id.strip().zfill(3) if ticket_id.strip().isdigit() else ticket_id
    for directory in (TICKETS_DIR, LANDED_DIR):
        if not directory.exists():
            continue
        for path in directory.glob(f"{tid}-*.md"):
            return path
    return None


def rewrite_ticket(
    path: Path,
    *,
    new_cluster: str | None,
    new_initiatives: list[str] | None,
) -> tuple[bool, list[str]]:
    """Rewrite the frontmatter of one ticket file.

    Returns (changed, notes). `changed=False` means the file already had the
    target values (no-op). `notes` is human-readable per-field status.
    """
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines(keepends=True)
    notes: list[str] = []

    if not lines or lines[0].rstrip() != "---":
        notes.append("SKIP: no frontmatter")
        return False, notes

    end_idx: int | None = None
    for i in range(1, len(lines)):
        if lines[i].rstrip() == "---":
            end_idx = i
            break
    if end_idx is None:
        notes.append("SKIP: frontmatter unterminated")
        return False, notes

    changed = False
    cluster_seen = False
    initiative_seen = False

    for i in range(1, end_idx):
        raw = lines[i].rstrip("\n").rstrip("\r")
        if new_cluster is not None and raw.startswith("cluster:"):
            cluster_seen = True
            m = CLUSTER_LINE_RE.match(raw)
            current = (m.group(2).strip() if m else "").rstrip()
            if current != new_cluster:
                lines[i] = f"cluster: {new_cluster}\n"
                notes.append(f"cluster: {current} → {new_cluster}")
                changed = True
            else:
                notes.append(f"cluster: unchanged ({new_cluster})")
        elif new_initiatives is not None and raw.startswith("initiative:"):
            initiative_seen = True
            current = raw.split(":", 1)[1].strip()
            new_val = "[" + ", ".join(new_initiatives) + "]"
            if current != new_val:
                lines[i] = f"initiative: {new_val}\n"
                notes.append(f"initiative: {current} → {new_val}")
                changed = True
            else:
                notes.append(f"initiative: unchanged ({new_val})")

    # If cluster line wasn't seen, insert it after status:.
    if new_cluster is not None and not cluster_seen:
        for i in range(1, end_idx):
            if lines[i].startswith("status:"):
                lines.insert(i + 1, f"cluster: {new_cluster}\n")
                notes.append(f"cluster: (inserted) → {new_cluster}")
                changed = True
                end_idx += 1
                break

    # If initiative line wasn't seen, insert it after cluster:.
    if new_initiatives is not None and not initiative_seen:
        for i in range(1, end_idx):
            if lines[i].startswith("cluster:"):
                lines.insert(i + 1, f"initiative: [{', '.join(new_initiatives)}]\n")
                notes.append(f"initiative: (inserted)")
                changed = True
                end_idx += 1
                break

    if changed:
        path.write_text("".join(lines), encoding="utf-8")
    return changed, notes


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("proposals", type=Path, help="Proposals YAML file")
    ap.add_argument("--dry-run", action="store_true", help="Report changes without writing")
    args = ap.parse_args(argv)

    if not args.proposals.exists():
        print(f"apply_tags: {args.proposals} not found", file=sys.stderr)
        return 1

    data = yaml.safe_load(args.proposals.read_text(encoding="utf-8")) or {}
    proposals = data.get("proposals", [])

    applied = 0
    skipped = 0
    rejected = 0
    errors: list[str] = []

    for p in proposals:
        if not p.get("approve"):
            rejected += 1
            continue
        tid = str(p.get("ticket_id", ""))
        # If the YAML carries an explicit path, prefer it (unique per file —
        # robust to legacy date-prefix ids that are shared across multiple
        # files). Otherwise resolve by id (robust to slug rename).
        path = None
        yaml_path = p.get("path")
        if yaml_path:
            candidate = REPO_ROOT / yaml_path
            if candidate.exists():
                path = candidate
        if path is None:
            path = resolve_path_by_id(tid)
        if path is None:
            errors.append(f"ticket {tid}: no file found by id, path={p.get('path')}")
            continue
        rel_path = str(path.relative_to(REPO_ROOT))

        new_cluster = p.get("proposed_cluster") or None
        new_inits = p.get("proposed_initiatives")
        if new_inits is None:
            new_inits = None
        elif not isinstance(new_inits, list):
            new_inits = list(new_inits)

        if args.dry_run:
            changed, notes = (
                bool(new_cluster) or bool(new_inits),
                [f"DRY: would set cluster={new_cluster} initiatives={new_inits}"],
            )
        else:
            changed, notes = rewrite_ticket(path, new_cluster=new_cluster, new_initiatives=new_inits)

        prefix = "applied" if changed else "no-op "
        print(f"  {prefix} [{p.get('ticket_id')}] {rel_path}")
        for n in notes:
            print(f"      {n}")
        if changed:
            applied += 1
        else:
            skipped += 1

    print(file=sys.stderr)
    print(f"summary: applied={applied} no-op={skipped} rejected={rejected}", file=sys.stderr)
    if errors:
        print(f"errors ({len(errors)}):", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
