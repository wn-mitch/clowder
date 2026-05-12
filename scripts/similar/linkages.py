#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "fastembed>=0.4",
#     "numpy>=1.26",
# ]
# ///
"""`just similar-linkages` — surface ticket pairs that are conceptually
adjacent but aren't formally linked.

Scans every ticket centroid against every other ticket centroid in the
embedding index, and emits the top-K pairs that:
  - score above `--threshold` (default 0.75)
  - don't currently cross-reference each other in body text
  - don't already share a `blocked-by` / `supersedes` relationship

Default scope is open tickets ↔ {open tickets, landed}. The pairs are
the ones that *should* be linked but aren't — reviewing them is one of
the highest-leverage uses of an embedding index over a ticket corpus,
because the formal `cluster:` field is too coarse to catch
cross-epic overlaps and authoring-time forgets.

Output: standard envelope (see scripts/logq/envelope.py).
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "logq"))

from envelope import Envelope, emit                    # noqa: E402  type: ignore[import-not-found]
from retrieve import Index, load_index, weighted_centroid_from_rows                 # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]


@dataclass
class TicketView:
    """One row in the per-ticket centroid table.

    `vector` is the mean of all this ticket's chunks' embeddings (then
    L2-normalized so dot-product is cosine). `references` is the set
    of *other* ticket ids this ticket already mentions in its body —
    parsed from any 2–3 digit integer that resolves to a known
    ticket id. `blocked_by` and `supersedes` come from frontmatter.
    """
    ticket_id: int
    title: str
    status: str
    cluster: str | None
    landed_on: str | None
    source_kind: str        # "tickets" or "landed"
    source_path: str
    vector: np.ndarray
    references: set[int]    # ticket ids mentioned in body
    blocked_by: set[int]
    supersedes: set[int]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Find ticket pairs that should be linked but aren't.",
    )
    parser.add_argument(
        "--threshold", type=float, default=0.75,
        help="Minimum cosine similarity for a pair to qualify (default: 0.75).",
    )
    parser.add_argument(
        "--top-k", type=int, default=30,
        help="Maximum pairs to return (default: 30).",
    )
    parser.add_argument(
        "--cross-cluster", action="store_true",
        help="Restrict to pairs whose clusters differ (catches cross-epic overlaps).",
    )
    parser.add_argument(
        "--include-landed-pairs", action="store_true",
        help="Also surface landed↔landed pairs. Default: only pairs with at "
             "least one open ticket — landed↔landed adds noise without action.",
    )
    parser.add_argument(
        "--ticket", type=int, default=None,
        help="Restrict to pairs involving this specific ticket id.",
    )
    parser.add_argument(
        "--text", action="store_true",
        help="Emit text envelope instead of JSON.",
    )
    args = parser.parse_args()

    try:
        idx = load_index(REPO_ROOT)
    except FileNotFoundError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2

    log = lambda *a: print(*a, file=sys.stderr, flush=True)

    log(f"loading {len(idx.chunks)} index chunks...")
    tickets = build_ticket_views(idx)
    log(f"built {len(tickets)} ticket centroids "
        f"({sum(1 for t in tickets if t.source_kind == 'tickets')} open, "
        f"{sum(1 for t in tickets if t.source_kind == 'landed')} landed)")

    pairs = score_pairs(
        tickets,
        threshold=args.threshold,
        cross_cluster_only=args.cross_cluster,
        include_landed_pairs=args.include_landed_pairs,
        focus_id=args.ticket,
    )
    log(f"found {len(pairs)} candidate pairs above threshold {args.threshold}")

    pairs.sort(key=lambda p: -p["score"])
    pairs = pairs[: args.top_k]

    env = Envelope(
        query={
            "threshold": args.threshold,
            "top_k": args.top_k,
            "cross_cluster_only": args.cross_cluster,
            "include_landed_pairs": args.include_landed_pairs,
            "focus_ticket": args.ticket,
        },
        scan_stats={
            "ticket_centroids": len(tickets),
            "candidate_pairs": len(pairs),
            "returned": len(pairs),
            "narrow_by": ["threshold", "cross_cluster", "ticket"],
        },
        results=pairs,
        narrative=_make_narrative(pairs, tickets, args),
        next=_suggest_next(args, len(pairs)),
    )
    emit(env, fmt="text" if args.text else "json")
    return 0 if pairs else 1


# ── ticket assembly ─────────────────────────────────────────────────────────

def build_ticket_views(idx: Index) -> list[TicketView]:
    """Group index chunks by ticket and compute per-ticket centroids.

    Walks the ticket bodies on disk to extract `references`,
    `blocked_by`, `supersedes` — that data isn't in the centroid;
    it's what we filter pairs against later."""
    chunks_by_id: dict[int, list[tuple[int, dict]]] = {}
    for row, chunk in enumerate(idx.chunks):
        if chunk["source_kind"] not in ("tickets", "landed"):
            continue
        md = chunk.get("metadata", {}) or {}
        tid = md.get("ticket_id")
        if not isinstance(tid, int):
            try:
                tid = int(tid) if tid is not None else None
            except (TypeError, ValueError):
                tid = None
        if tid is None:
            continue
        chunks_by_id.setdefault(tid, []).append((row, chunk))

    all_ids = set(chunks_by_id.keys())

    views: list[TicketView] = []
    for tid, rows in chunks_by_id.items():
        rep_chunk = rows[0][1]
        md = rep_chunk.get("metadata", {}) or {}
        rel_path = rep_chunk["source_path"]
        body = (REPO_ROOT / rel_path).read_text(encoding="utf-8")
        refs = _parse_references(body, all_ids, exclude=tid)
        blocked_by, supersedes = _parse_frontmatter_links(body, all_ids, exclude=tid)

        # Section-weighted centroid (see retrieve.SECTION_WEIGHTS) so
        # tickets are characterized by their intent (Why / Scope /
        # Approach) rather than smudged by process boilerplate.
        vec_rows = [r for r, _ in rows]
        centroid = weighted_centroid_from_rows(idx, vec_rows)

        views.append(TicketView(
            ticket_id=tid,
            title=str(md.get("title", "")),
            status=str(md.get("status", "?")),
            cluster=md.get("cluster") if md.get("cluster") not in (None, "—") else None,
            landed_on=md.get("landed_on"),
            source_kind=rep_chunk["source_kind"],
            source_path=rel_path,
            vector=centroid.astype(np.float32),
            references=refs,
            blocked_by=blocked_by,
            supersedes=supersedes,
        ))
    return views


_INTEGER_RE = re.compile(r"\b(\d{2,4})\b")
_FRONTMATTER_BLOCKED_RE = re.compile(r"^blocked-by:\s*\[(.*?)\]", re.MULTILINE)
_FRONTMATTER_SUPERSEDES_RE = re.compile(r"^supersedes:\s*\[(.*?)\]", re.MULTILINE)


def _parse_references(body: str, all_ids: set[int], *, exclude: int) -> set[int]:
    """Pull every integer from body that matches a known ticket id.

    Keeps it loose: any 2–4 digit number that resolves to a real
    ticket id counts as a reference. False positives (e.g. "30%"
    happens to be ticket 30) just mean we filter the pair as
    "already linked" — better than missing real cross-references."""
    out: set[int] = set()
    for m in _INTEGER_RE.finditer(body):
        n = int(m.group(1))
        if n in all_ids and n != exclude:
            out.add(n)
    return out


def _parse_frontmatter_links(
    body: str, all_ids: set[int], *, exclude: int,
) -> tuple[set[int], set[int]]:
    """Pull `blocked-by:` and `supersedes:` from frontmatter."""
    def _parse_list(match: re.Match | None) -> set[int]:
        if not match:
            return set()
        raw = match.group(1).strip()
        if not raw:
            return set()
        out = set()
        for part in raw.split(","):
            try:
                n = int(part.strip())
                if n in all_ids and n != exclude:
                    out.add(n)
            except ValueError:
                continue
        return out

    return (
        _parse_list(_FRONTMATTER_BLOCKED_RE.search(body)),
        _parse_list(_FRONTMATTER_SUPERSEDES_RE.search(body)),
    )


# ── pair scoring ────────────────────────────────────────────────────────────

def score_pairs(
    tickets: list[TicketView],
    *,
    threshold: float,
    cross_cluster_only: bool,
    include_landed_pairs: bool,
    focus_id: int | None,
) -> list[dict[str, Any]]:
    """Compute pairwise similarity, filter, and shape pair records.

    O(N²) similarity scan over the centroid matrix is fine at this
    corpus size — ~250 tickets means 31k pairs, all in a single
    numpy dot product. The cost is in the per-pair filter (dict
    lookups, set membership checks)."""
    if not tickets:
        return []

    matrix = np.stack([t.vector for t in tickets])  # (N, D), all unit-norm
    sims = matrix @ matrix.T                         # (N, N) cosine
    n = len(tickets)

    out: list[dict[str, Any]] = []
    for i in range(n):
        a = tickets[i]
        if focus_id is not None and a.ticket_id != focus_id:
            # When --ticket is set, we still want pairs *involving* that
            # ticket, so check against the focus on either side below.
            pass
        for j in range(i + 1, n):
            b = tickets[j]
            if focus_id is not None and focus_id not in (a.ticket_id, b.ticket_id):
                continue

            score = float(sims[i, j])
            if score < threshold:
                continue

            # Filter: skip already-linked pairs.
            if b.ticket_id in a.references or a.ticket_id in b.references:
                continue
            if b.ticket_id in a.blocked_by or a.ticket_id in b.blocked_by:
                continue
            if b.ticket_id in a.supersedes or a.ticket_id in b.supersedes:
                continue

            # Filter: cluster constraints.
            if cross_cluster_only and a.cluster == b.cluster:
                continue

            # Filter: landed↔landed unless requested.
            if (a.source_kind == "landed" and b.source_kind == "landed"
                    and not include_landed_pairs):
                continue

            out.append(_pair_record(a, b, score))

    return out


def _pair_record(a: TicketView, b: TicketView, score: float) -> dict[str, Any]:
    pair_id = f"{min(a.ticket_id, b.ticket_id)}<>{max(a.ticket_id, b.ticket_id)}"
    return {
        "id": pair_id,
        "score": round(score, 4),
        "a": _ticket_summary(a),
        "b": _ticket_summary(b),
        "cross_cluster": a.cluster != b.cluster,
        "summary": (
            f"{a.ticket_id} ({a.cluster or '—'}, {a.status}) "
            f"↔ {b.ticket_id} ({b.cluster or '—'}, {b.status}) — "
            f"{_short(a.title)} ↔ {_short(b.title)}"
        ),
    }


def _ticket_summary(t: TicketView) -> dict[str, Any]:
    return {
        "id": t.ticket_id,
        "title": t.title,
        "status": t.status,
        "cluster": t.cluster,
        "source_kind": t.source_kind,
        "path": t.source_path,
        "landed_on": t.landed_on,
    }


def _short(s: str, max_len: int = 60) -> str:
    if len(s) <= max_len:
        return s
    return s[: max_len - 1].rstrip() + "…"


def _make_narrative(
    pairs: list[dict[str, Any]],
    tickets: list[TicketView],
    args,
) -> str:
    if not pairs:
        return (
            f"no candidate pairs above threshold {args.threshold}. Try "
            f"lowering with --threshold 0.7, or relax filters by dropping "
            f"--cross-cluster."
        )
    cross = sum(1 for p in pairs if p["cross_cluster"])
    open_pairs = sum(
        1 for p in pairs
        if p["a"]["source_kind"] == "tickets" or p["b"]["source_kind"] == "tickets"
    )
    return (
        f"{len(pairs)} pairs above threshold {args.threshold} "
        f"({cross} cross-cluster, {open_pairs} involving an open ticket). "
        f"Each pair has no current cross-reference in body, blocked-by, "
        f"or supersedes — these are candidates for new linkage."
    )


def _suggest_next(args, n_pairs: int) -> list[str]:
    out = []
    if args.threshold > 0.7:
        out.append(f"just similar-linkages --threshold {args.threshold - 0.05:.2f}")
    if not args.cross_cluster:
        out.append("just similar-linkages --cross-cluster")
    if n_pairs == args.top_k:
        out.append(f"just similar-linkages --top-k {args.top_k * 2}")
    if args.ticket is None and n_pairs > 0:
        out.append("just similar-linkages --ticket <id>  # drill into one ticket")
    return out


if __name__ == "__main__":
    raise SystemExit(main())
