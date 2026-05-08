"""Library helper for suggesting related tickets given a free-text
query — the exact thing `just open-ticket` wants to print right after
creating a new ticket so the author can decide which existing tickets
to formally link.

Distinct from `linkages.py` (which scans the full ticket corpus for
unlinked pairs) and `similar.py` (which is the user-facing CLI for
single-query retrieval). This module is library-only — it's the
function `create_ticket.py` calls.

Designed to fail soft. If the embedding index doesn't exist, fastembed
isn't installed, or any other piece of the pipeline isn't ready, the
function returns `[]` and the caller falls back to silent. We don't
want a missing index to block ticket creation.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]


def suggest_related_tickets(
    query_text: str,
    *,
    top_k: int = 5,
    repo_root: Path = REPO_ROOT,
    threshold: float = 0.6,
) -> list[dict[str, Any]]:
    """Return up to `top_k` ticket candidates whose chunks are
    semantically nearest to `query_text`.

    Group by ticket so each ticket appears at most once (with its
    best-matching chunk's score). `threshold` filters out weak hits;
    the default 0.6 is permissive because we'd rather over-suggest
    than under-suggest at ticket-open time — the cost of an extra
    suggestion line is one second of skim, vs. the cost of missing
    a real adjacency.

    Returns `[]` on any failure path (no index, missing dep, empty
    corpus). Suggestions are rendered into a strings by the caller —
    we don't print here so the function stays a pure library helper.
    """
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from embed import embed_batch                        # noqa: PLC0415
        from retrieve import load_index                      # noqa: PLC0415
    except ImportError:
        return []

    try:
        idx = load_index(repo_root)
    except FileNotFoundError:
        return []

    try:
        query_vec = embed_batch([query_text])[0]
    except Exception:
        return []

    import numpy as np                                       # noqa: PLC0415

    # Restrict to ticket+landed chunks only — we're suggesting cross-
    # references for a new open ticket, not surfacing system-doc or
    # DSE-doc-comment hits (those are valuable for `just similar` but
    # noisy for "what tickets relate to this title?").
    mask = np.array([
        c.get("source_kind") in ("tickets", "landed") for c in idx.chunks
    ], dtype=bool)
    if not mask.any():
        return []

    sims = idx.vectors @ query_vec.astype(np.float32, copy=False)
    sims = np.where(mask, sims, -np.inf)

    # Group by ticket_id, keeping the highest-scoring chunk per ticket.
    best_by_ticket: dict[int, tuple[float, dict]] = {}
    for i, chunk in enumerate(idx.chunks):
        if not mask[i]:
            continue
        score = float(sims[i])
        if score < threshold:
            continue
        md = chunk.get("metadata", {}) or {}
        tid = md.get("ticket_id")
        if not isinstance(tid, int):
            try:
                tid = int(tid) if tid is not None else None
            except (TypeError, ValueError):
                continue
        if tid is None:
            continue
        prev = best_by_ticket.get(tid)
        if prev is None or score > prev[0]:
            best_by_ticket[tid] = (score, chunk)

    candidates = sorted(best_by_ticket.items(), key=lambda kv: -kv[1][0])
    out: list[dict[str, Any]] = []
    for tid, (score, chunk) in candidates[:top_k]:
        md = chunk.get("metadata", {}) or {}
        out.append({
            "ticket_id": tid,
            "title": md.get("title", ""),
            "status": md.get("status", "?"),
            "cluster": md.get("cluster"),
            "source_kind": chunk.get("source_kind"),
            "score": round(score, 4),
            "section": chunk.get("section"),
        })
    return out


def render_suggestions_lines(
    suggestions: list[dict[str, Any]],
    *,
    indent: str = "  ",
) -> list[str]:
    """Format suggestions for stderr printing in `just open-ticket`.

    Returns a list of human-readable lines (no terminal escapes).
    Caller decides whether to actually print them — keeping
    formatting separate from policy keeps the helper testable."""
    if not suggestions:
        return []
    lines = [f"{indent}suggested related work (run `just similar-linkages "
             f"--ticket <new-id>` after fleshing out ## Why for sharper hits):"]
    for s in suggestions:
        cluster_bit = f", {s['cluster']}" if s.get("cluster") else ""
        kind_marker = "·" if s["source_kind"] == "tickets" else "✓"
        title = _short(s["title"], 70)
        lines.append(
            f"{indent}  {kind_marker} {s['ticket_id']:>3} "
            f"({s['status']}{cluster_bit}, {s['score']:.2f}) — {title}"
        )
    return lines


def _short(s: str, max_len: int) -> str:
    if len(s) <= max_len:
        return s
    return s[: max_len - 1].rstrip() + "…"
