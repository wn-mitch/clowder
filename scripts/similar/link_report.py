#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "fastembed>=0.4",
#     "numpy>=1.26",
# ]
# ///
"""`just similar-link-report` — bulk linkage curation across all open
tickets.

Produces two outputs from one pass over the embedding index:

  1. `docs/open-work/_linkages.md` — single navigable report ranking
     open tickets by linkage density, grouped for "what's next"
     reasoning. Top-level navigation aid; review and act on it,
     then delete or regenerate.

  2. A `## Related work` block injected into each open ticket whose
     candidates clear the threshold. Block is wrapped in a marker
     comment so it's distinguishable from human-curated prose.

The bulk edit is conservative: threshold 0.78, top-3 candidates per
ticket, only same-or-cross-cluster pairs (no landed↔landed). The
filters in `linkages.py::score_pairs` already exclude pairs already
cross-referenced via body / blocked-by / supersedes.

Re-running is idempotent: existing `## Related work` blocks (matched
via the marker comment) are replaced, not duplicated.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from collections import defaultdict
from datetime import date
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

from linkages import TicketView, build_ticket_views, score_pairs    # noqa: E402
from retrieve import load_index                                      # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
TICKETS_DIR = REPO_ROOT / "docs" / "open-work" / "tickets"
REPORT_PATH = REPO_ROOT / "docs" / "open-work" / "_linkages.md"

# Markers — anything between START and END is treated as auto-generated
# and replaced on re-run. Anything outside is preserved.
SECTION_START = "<!-- linkages:start -->"
SECTION_END = "<!-- linkages:end -->"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Bulk linkage curation across open tickets.",
    )
    parser.add_argument(
        "--threshold", type=float, default=0.78,
        help="Minimum cosine similarity for a candidate to qualify (default: 0.78).",
    )
    parser.add_argument(
        "--per-ticket", type=int, default=3,
        help="Max suggestions per open ticket (default: 3).",
    )
    parser.add_argument(
        "--report-only", action="store_true",
        help="Write the top-level report but don't edit ticket files.",
    )
    parser.add_argument(
        "--no-report", action="store_true",
        help="Edit ticket files but skip the top-level report.",
    )
    args = parser.parse_args()

    log = lambda *a: print(*a, file=sys.stderr, flush=True)

    log("loading index...")
    idx = load_index(REPO_ROOT)
    log(f"  {len(idx.chunks)} chunks")

    log("building ticket centroids...")
    tickets = build_ticket_views(idx)
    open_tickets = [t for t in tickets if t.source_kind == "tickets"]
    log(f"  {len(tickets)} total, {len(open_tickets)} open")

    log(f"scoring pairs (threshold {args.threshold})...")
    pairs = score_pairs(
        tickets,
        threshold=args.threshold,
        cross_cluster_only=False,
        include_landed_pairs=False,
        focus_id=None,
    )
    log(f"  {len(pairs)} candidate pairs above threshold")

    # Index pairs by ticket id (each pair appears under both endpoints).
    candidates_by_id: dict[int, list[dict]] = defaultdict(list)
    for p in pairs:
        for end in ("a", "b"):
            other = "b" if end == "a" else "a"
            tid = p[end]["id"]
            candidates_by_id[tid].append({
                "score": p["score"],
                "other": p[other],
                "cross_cluster": p["cross_cluster"],
            })
    # Sort each ticket's candidates by score, take top-K.
    for tid in candidates_by_id:
        candidates_by_id[tid].sort(key=lambda c: -c["score"])
        candidates_by_id[tid] = candidates_by_id[tid][: args.per_ticket]

    open_ticket_ids = {t.ticket_id for t in open_tickets}

    # Edit ticket files first (so the report can reference what was added).
    edits = 0
    if not args.report_only:
        for t in open_tickets:
            cands = candidates_by_id.get(t.ticket_id, [])
            if not cands:
                continue
            edited = _inject_related_section(t, cands)
            if edited:
                edits += 1
        log(f"edited {edits} open tickets with `## Related work` sections")

    # Top-level report.
    if not args.no_report:
        report = _build_report(
            open_tickets, candidates_by_id, threshold=args.threshold,
        )
        REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
        REPORT_PATH.write_text(report, encoding="utf-8")
        log(f"wrote {REPORT_PATH.relative_to(REPO_ROOT)} "
            f"({len(report.splitlines())} lines)")

    return 0


# ── per-ticket injection ────────────────────────────────────────────────────

_LOG_HEADING_RE = re.compile(r"^##\s+Log\s*$", re.MULTILINE)
_RELATED_HEADING_RE = re.compile(r"^##\s+Related work\s*$", re.MULTILINE)


def _inject_related_section(t: TicketView, cands: list[dict]) -> bool:
    """Insert (or replace) a `## Related work` section in this ticket.

    Returns True if the file was modified. Idempotent: re-running on a
    ticket whose section already exists overwrites only the auto-marked
    block, leaving any human additions outside the markers untouched.
    """
    path = REPO_ROOT / t.source_path
    body = path.read_text(encoding="utf-8")

    new_section = _render_section(cands)

    if _RELATED_HEADING_RE.search(body):
        # Section exists — replace just the auto-marked block within it.
        # Find SECTION_START / SECTION_END inside the existing section
        # and swap. If markers are missing, append a fresh marked block
        # at the end of the existing section (before the next ##).
        new_body = _replace_marked_block(body, new_section)
    else:
        # No `## Related work` yet — insert before `## Log` if present,
        # else append at end.
        m = _LOG_HEADING_RE.search(body)
        if m:
            insertion = f"## Related work\n\n{new_section}\n"
            new_body = body[: m.start()] + insertion + body[m.start() :]
        else:
            new_body = body.rstrip() + "\n\n## Related work\n\n" + new_section + "\n"

    if new_body == body:
        return False
    path.write_text(new_body, encoding="utf-8")
    return True


def _render_section(cands: list[dict]) -> str:
    """Render the auto-marked block contents (between START and END)."""
    today = date.today().isoformat()
    lines = [
        SECTION_START,
        f"<!-- generated by `just similar-link-report` on {today} — "
        f"review and prune; pairs above threshold that aren't already "
        f"cross-referenced. -->",
        "",
    ]
    for c in cands:
        o = c["other"]
        cluster = o.get("cluster") or "—"
        cross_marker = " (cross-cluster)" if c["cross_cluster"] else ""
        kind_marker = "·" if o["source_kind"] == "tickets" else "✓ landed"
        title = _short(o["title"], 80)
        lines.append(
            f"- {kind_marker} **{o['id']:>3}** "
            f"({o['status']}, {cluster}, score {c['score']:.2f}{cross_marker}) — {title}"
        )
    lines.append("")
    lines.append(SECTION_END)
    return "\n".join(lines)


def _replace_marked_block(body: str, new_section: str) -> str:
    """Replace the auto-marked block in `body` with `new_section`. If
    the markers don't exist (someone added a human-curated `## Related
    work` section by hand), insert the marked block at the end of that
    section, before the next `##` heading."""
    start_idx = body.find(SECTION_START)
    end_idx = body.find(SECTION_END)
    if start_idx != -1 and end_idx != -1 and end_idx > start_idx:
        before = body[:start_idx]
        after = body[end_idx + len(SECTION_END):]
        return before + new_section + after

    # Markers missing — find `## Related work` and insert at its end.
    m = _RELATED_HEADING_RE.search(body)
    if not m:
        return body  # shouldn't reach here; caller checked
    section_start = m.end()
    next_heading = re.search(r"\n##\s+", body[section_start:])
    if next_heading:
        section_end = section_start + next_heading.start()
    else:
        section_end = len(body)
    section_body = body[section_start:section_end]
    insertion = f"\n\n{new_section}\n"
    return body[:section_start] + section_body.rstrip() + insertion + body[section_end:]


# ── top-level report ────────────────────────────────────────────────────────

def _build_report(
    open_tickets: list[TicketView],
    candidates_by_id: dict[int, list[dict]],
    *,
    threshold: float,
) -> str:
    today = date.today().isoformat()
    by_id = {t.ticket_id: t for t in open_tickets}

    # Density: how many high-confidence candidates each open ticket has.
    densities = sorted(
        ((tid, len(cands)) for tid, cands in candidates_by_id.items()
         if tid in by_id),
        key=lambda kv: -kv[1],
    )

    lines: list[str] = []
    lines.append(f"# Open-ticket linkage suggestions — {today}")
    lines.append("")
    lines.append(
        f"Auto-generated by `just similar-link-report` over the "
        f"embedding index (BAAI/bge-small-en-v1.5, threshold "
        f"{threshold}). Each open ticket lists adjacent tickets that "
        f"score above {threshold} cosine similarity AND don't already "
        f"cross-reference this one (via body, blocked-by, or "
        f"supersedes). Use this to navigate \"what's next\" — review "
        f"and add real linkages where appropriate, then re-run to "
        f"refresh."
    )
    lines.append("")

    # ── neighborhood density top-N ──
    lines.append("## Neighborhood density (top-20)")
    lines.append("")
    lines.append(
        "Open tickets with the most unlinked high-confidence neighbors. "
        "These are the most \"central\" tickets in the open-work graph — "
        "either over-broad in scope or genuine cross-cutting concerns "
        "worth surfacing first."
    )
    lines.append("")
    lines.append("| # | id | status | cluster | candidates | title |")
    lines.append("|---|---|---|---|---|---|")
    for rank, (tid, n) in enumerate(densities[:20], 1):
        t = by_id[tid]
        lines.append(
            f"| {rank} | {tid} | {t.status} | {t.cluster or '—'} | "
            f"{n} | {_short(t.title, 70)} |"
        )
    lines.append("")

    # ── cluster overlap matrix ──
    lines.append("## Cross-cluster overlap")
    lines.append("")
    lines.append(
        "Pairs whose two tickets are in different clusters (or one in "
        "`cluster: null`). These are the conceptual overlaps the formal "
        "`cluster:` field can't catch — the highest-leverage linkage "
        "candidates."
    )
    lines.append("")
    cross_pairs: list[tuple[int, int, float, str, str]] = []
    seen_pairs: set[tuple[int, int]] = set()
    for tid, cands in candidates_by_id.items():
        if tid not in by_id:
            continue
        a = by_id[tid]
        for c in cands:
            if not c["cross_cluster"]:
                continue
            other_id = c["other"]["id"]
            pair_key = (min(tid, other_id), max(tid, other_id))
            if pair_key in seen_pairs:
                continue
            seen_pairs.add(pair_key)
            cross_pairs.append((
                pair_key[0], pair_key[1], c["score"],
                f"{a.cluster or '—'} ↔ {c['other'].get('cluster') or '—'}",
                f"{_short(a.title, 50)} ↔ {_short(c['other']['title'], 50)}",
            ))
    cross_pairs.sort(key=lambda p: -p[2])
    lines.append("| score | a ↔ b | clusters | titles |")
    lines.append("|---|---|---|---|")
    for a_id, b_id, score, clusters, titles in cross_pairs[:30]:
        lines.append(f"| {score:.2f} | {a_id} ↔ {b_id} | {clusters} | {titles} |")
    lines.append("")

    # ── per-ticket detail ──
    lines.append("## Per-ticket suggestions")
    lines.append("")
    lines.append(
        "Each open ticket below has top-3 candidates above the "
        "threshold, sorted by score. Same data lives in each ticket's "
        "`## Related work` section between `<!-- linkages:start -->` "
        "and `<!-- linkages:end -->` markers — re-running this tool "
        "replaces only the marked block, preserving any prose you add "
        "outside it."
    )
    lines.append("")

    for t in sorted(open_tickets, key=lambda x: x.ticket_id):
        cands = candidates_by_id.get(t.ticket_id, [])
        if not cands:
            continue
        lines.append(f"### {t.ticket_id} — {t.title}")
        lines.append(
            f"`{t.status}` · cluster: `{t.cluster or '—'}` · "
            f"`{t.source_path}`"
        )
        lines.append("")
        for c in cands:
            o = c["other"]
            cluster = o.get("cluster") or "—"
            cross_marker = " (cross-cluster)" if c["cross_cluster"] else ""
            kind_marker = "·" if o["source_kind"] == "tickets" else "✓"
            title = _short(o["title"], 70)
            lines.append(
                f"- {kind_marker} **{o['id']}** "
                f"({o['status']}, {cluster}, {c['score']:.2f}"
                f"{cross_marker}) — {title}"
            )
        lines.append("")

    # Footer.
    lines.append("---")
    lines.append("")
    no_neighbors = [
        t for t in open_tickets if t.ticket_id not in candidates_by_id
    ]
    lines.append(
        f"_{len(open_tickets)} open tickets · "
        f"{sum(len(c) for c in candidates_by_id.values()) // 2} unique "
        f"candidate pairs above threshold {threshold} · "
        f"{len(no_neighbors)} open tickets with no adjacent neighbors "
        f"(probably scope-narrow or genuinely orphan)._"
    )
    if no_neighbors:
        lines.append("")
        lines.append("**Open tickets with no above-threshold neighbors:**")
        lines.append("")
        for t in sorted(no_neighbors, key=lambda x: x.ticket_id):
            lines.append(
                f"- {t.ticket_id} ({t.status}, {t.cluster or '—'}) "
                f"— {_short(t.title, 70)}"
            )

    return "\n".join(lines) + "\n"


def _short(s: str, max_len: int) -> str:
    if len(s) <= max_len:
        return s
    return s[: max_len - 1].rstrip() + "…"


if __name__ == "__main__":
    raise SystemExit(main())
