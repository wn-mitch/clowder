#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Render the open-work ticket dependency graph as an ASCII or Mermaid tree.

Edges come from each ticket's `blocked-by` frontmatter. Default direction
is downward — roots are tickets that block others, children are the
tickets they unblock. `--upward` flips it so roots are blocked tickets
and children are their blockers.

The graph is a DAG, not a tree: a ticket can have multiple blockers. In
ASCII mode, multi-parent nodes render fully under the first parent and
as `[NNN ↑]` back-references under subsequent parents (mirrors how
`tree(1)` handles hardlinks).

Usage:
    uv run scripts/open_work_tree.py
    uv run scripts/open_work_tree.py --upward
    uv run scripts/open_work_tree.py --root 011
    uv run scripts/open_work_tree.py --format mermaid
    uv run scripts/open_work_tree.py --status ready,blocked --cluster ai-substrate
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from generate_open_work import Ticket, load_tickets  # noqa: E402


OPEN_STATUSES = {"ready", "in-progress", "blocked", "parked"}

ANSI = {
    "ready": "\033[32m",       # green
    "in-progress": "\033[33m", # yellow
    "blocked": "\033[31m",     # red
    "parked": "\033[2m",       # dim
    "done": "\033[34m",        # blue
    "dropped": "\033[2;31m",   # dim red
    "reset": "\033[0m",
    "ref": "\033[2m",          # dim for back-refs
}


def normalize_id(raw) -> str:
    """3-digit zero-pad numeric ids, leave non-numeric ids as-is."""
    if isinstance(raw, int):
        return f"{raw:03d}"
    s = str(raw).strip()
    try:
        return f"{int(s):03d}"
    except ValueError:
        return s


def load_all(repo_root: Path) -> dict[str, Ticket]:
    """Merge active tickets + landed tickets, active wins on id collision."""
    landed = load_tickets(repo_root / "docs" / "open-work" / "landed")
    active = load_tickets(repo_root / "docs" / "open-work" / "tickets")
    merged: dict[str, Ticket] = {}
    for t in landed:
        merged[normalize_id(t.frontmatter.get("id"))] = t
    for t in active:
        merged[normalize_id(t.frontmatter.get("id"))] = t
    return merged


def build_edges(tickets: dict[str, Ticket]) -> tuple[dict[str, list[str]], dict[str, list[str]]]:
    """Return (blocks, blocked_by) adjacency maps keyed by normalized id."""
    blocks: dict[str, list[str]] = {tid: [] for tid in tickets}
    blocked_by: dict[str, list[str]] = {tid: [] for tid in tickets}
    for tid, t in tickets.items():
        for raw_blocker in t.blocked_by:
            bid = normalize_id(raw_blocker)
            blocked_by[tid].append(bid)
            blocks.setdefault(bid, []).append(tid)
    return blocks, blocked_by


def detect_cycles(adj: dict[str, list[str]]) -> list[tuple[str, str]]:
    """DFS coloring; returns the list of back-edges that close cycles."""
    WHITE, GRAY, BLACK = 0, 1, 2
    color: dict[str, int] = {n: WHITE for n in adj}
    back_edges: list[tuple[str, str]] = []

    def visit(n: str, stack: list[str]) -> None:
        color[n] = GRAY
        stack.append(n)
        for child in adj.get(n, []):
            if color.get(child, WHITE) == GRAY:
                back_edges.append((n, child))
            elif color.get(child, WHITE) == WHITE:
                visit(child, stack)
        stack.pop()
        color[n] = BLACK

    for node in list(adj.keys()):
        if color[node] == WHITE:
            visit(node, [])
    return back_edges


def label(tid: str, tickets: dict[str, Ticket], use_color: bool) -> str:
    if tid not in tickets:
        body = f"[{tid} landed] (archive)"
        return f"{ANSI['done']}{body}{ANSI['reset']}" if use_color else body
    t = tickets[tid]
    status = t.status
    cluster = t.cluster
    bits = [tid, status]
    if cluster:
        bits.append(str(cluster))
    head = "·".join(bits)
    body = f"[{head}] {t.title}"
    if use_color:
        color = ANSI.get(status, "")
        return f"{color}{body}{ANSI['reset']}" if color else body
    return body


def filter_tickets(
    tickets: dict[str, Ticket],
    statuses: set[str] | None,
    cluster: str | None,
) -> set[str]:
    """Ids to consider as graph members; landed-only blockers stay reachable as terminal stubs."""
    out: set[str] = set()
    for tid, t in tickets.items():
        if statuses is not None and t.status not in statuses:
            continue
        if cluster is not None and (t.cluster or "") != cluster:
            continue
        out.add(tid)
    return out


def render_ascii(
    roots: list[str],
    children_of: dict[str, list[str]],
    tickets: dict[str, Ticket],
    members: set[str],
    skip_edges: set[tuple[str, str]],
    use_color: bool,
) -> list[str]:
    out: list[str] = []
    rendered_full: set[str] = set()

    def walk(tid: str, prefix: str, is_last: bool, is_root: bool) -> None:
        connector = "" if is_root else ("└── " if is_last else "├── ")
        is_back_ref = tid in rendered_full
        suffix = ""
        text = label(tid, tickets, use_color)
        if is_back_ref:
            suffix = " ↑"
            if use_color:
                text = f"{ANSI['ref']}[{tid} ↑]{ANSI['reset']}"
            else:
                text = f"[{tid} ↑]"
            suffix = ""
        out.append(prefix + connector + text + suffix)
        if is_back_ref:
            return
        rendered_full.add(tid)
        kids = [
            c
            for c in children_of.get(tid, [])
            if (tid, c) not in skip_edges and (c in members or c in tickets)
        ]
        new_prefix = prefix + ("" if is_root else ("    " if is_last else "│   "))
        for i, kid in enumerate(kids):
            walk(kid, new_prefix, i == len(kids) - 1, is_root=False)

    for root in roots:
        walk(root, "", True, is_root=True)
        out.append("")
    return out


def _mermaid_class(status: str) -> str:
    return "st_" + status.replace("-", "_")


def render_mermaid(
    edges: list[tuple[str, str]],
    tickets: dict[str, Ticket],
    nodes: set[str],
) -> list[str]:
    out = ["```mermaid", "flowchart TD"]
    for nid in sorted(nodes):
        if nid in tickets:
            t = tickets[nid]
            label_text = f"{nid}: {t.title}".replace('"', "'")
            style = t.status
        else:
            label_text = f"{nid}: (landed)"
            style = "done"
        out.append(f'    {nid}["{label_text}"]:::{_mermaid_class(style)}')
    for src, dst in edges:
        out.append(f"    {src} --> {dst}")
    out.append("    classDef st_ready fill:#cfc,stroke:#393")
    out.append("    classDef st_in_progress fill:#ffc,stroke:#cc3")
    out.append("    classDef st_blocked fill:#fcc,stroke:#c33")
    out.append("    classDef st_parked fill:#eee,stroke:#999")
    out.append("    classDef st_done fill:#ccf,stroke:#33c")
    out.append("    classDef st_dropped fill:#eee,stroke:#c66")
    out.append("```")
    return out


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--upward",
        action="store_true",
        help="Invert: roots are blocked tickets, children are their blockers.",
    )
    p.add_argument(
        "--status",
        type=str,
        default=None,
        help="Comma-separated statuses to include (default: open statuses). Use 'all' for every status.",
    )
    p.add_argument("--cluster", type=str, default=None)
    p.add_argument("--root", type=str, default=None, help="Print only the subtree at this id.")
    p.add_argument(
        "--format",
        choices=("ascii", "mermaid"),
        default="ascii",
    )
    p.add_argument("--no-color", action="store_true")
    return p.parse_args()


def main() -> int:
    args = parse_args()

    if args.status is None:
        statuses: set[str] | None = OPEN_STATUSES
    elif args.status == "all":
        statuses = None
    else:
        statuses = {s.strip() for s in args.status.split(",") if s.strip()}

    tickets = load_all(REPO_ROOT)
    blocks, blocked_by = build_edges(tickets)

    direction = blocked_by if args.upward else blocks
    back_edges = detect_cycles(direction)
    if back_edges:
        print(
            f"warning: detected {len(back_edges)} cycle back-edge(s); "
            f"skipping during render: {back_edges}",
            file=sys.stderr,
        )
    skip_edges = set(back_edges)

    members = filter_tickets(tickets, statuses, args.cluster)

    if args.root is not None:
        root_id = normalize_id(args.root)
        if root_id not in tickets:
            print(f"error: root id {root_id!r} not found", file=sys.stderr)
            return 1
        roots = [root_id]
        members = members | {root_id}
    else:
        candidates = members
        roots = sorted(
            tid
            for tid in candidates
            if not any(p in candidates for p in (blocks if args.upward else blocked_by).get(tid, []))
            and direction.get(tid)
        )

    use_color = sys.stdout.isatty() and not args.no_color and os.environ.get("NO_COLOR") is None

    if args.format == "ascii":
        out_lines: list[str] = []
        out_lines.append(
            f"# Open-work dependency tree "
            f"({'upward: blocked → blockers' if args.upward else 'downward: blocker → unblocks'})"
        )
        out_lines.append("")
        if not roots:
            out_lines.append("(no edges to render)")
        else:
            out_lines.extend(render_ascii(roots, direction, tickets, members, skip_edges, use_color))

        if args.root is None:
            in_graph: set[str] = set(roots)
            stack = list(roots)
            while stack:
                n = stack.pop()
                for c in direction.get(n, []):
                    if c not in in_graph:
                        in_graph.add(c)
                        stack.append(c)
            orphans = sorted(
                tid for tid in members
                if tid not in in_graph
                and not blocks.get(tid)
                and not blocked_by.get(tid)
            )
            if orphans:
                out_lines.append("## Orphans (no dependency edges)")
                out_lines.append("")
                for tid in orphans:
                    out_lines.append(f"  {label(tid, tickets, use_color)}")
                out_lines.append("")

        print("\n".join(out_lines))
        return 0

    edges: list[tuple[str, str]] = []
    nodes: set[str] = set()
    visited: set[str] = set()

    def collect(node: str) -> None:
        if node in visited:
            return
        visited.add(node)
        nodes.add(node)
        for c in direction.get(node, []):
            if (node, c) in skip_edges:
                continue
            edges.append((node, c))
            collect(c)

    for r in roots:
        collect(r)
    if args.root is None:
        for tid in members:
            if tid not in nodes and not blocks.get(tid) and not blocked_by.get(tid):
                nodes.add(tid)

    print("\n".join(render_mermaid(edges, tickets, nodes)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
