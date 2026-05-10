#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "fastembed>=0.4",
#     "numpy>=1.26",
# ]
# ///
"""`just next` — embedding-based ticket recommender.

Picks a small ranked list of ready tickets that are semantically
adjacent to recent landings, current in-flight work, the
substrate-refactor epic, or an ad-hoc seed. Reads the existing index
at `logs/.embeddings/` — never rebuilds, never writes.

Five modes:

  just next                          # blend (default)
  just next --mode momentum          # last-N landed centroid
  just next --mode wip               # in-progress centroid
  just next --mode substrate         # AI-refactor alignment
  just next --mode seed --seed 256   # ticket-id seed
  just next --mode seed --seed "scent"   # free-text seed

Top-K defaults to 5; each result carries a one-line rationale naming
the source ticket / chunk it is most adjacent to. Source set members
of the query vector are excluded from the candidate set, so e.g.
`--mode wip` won't recommend tickets currently in-progress.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "logq"))

from chunkers import discover_corpus_files                                # noqa: E402
from embed import EMBEDDER_NAME, embed_batch                              # noqa: E402
from envelope import Envelope, emit                                       # noqa: E402  type: ignore[import-not-found]
from retrieve import Index, load_index, stale_files                       # noqa: E402


REPO_ROOT = Path(__file__).resolve().parents[2]
SUBSTRATE_SPEC_PATH = "docs/systems/ai-substrate-refactor.md"
SUBSTRATE_CLUSTERS = {"A", "B", "C", "D", "E"}

DEFAULT_TOP_K = 5
DEFAULT_LANDED_WINDOW = 5
DEFAULT_WEIGHTS = {"momentum": 0.5, "wip": 0.3, "substrate": 0.2}
NEAR_TIE_DELTA = 0.005


@dataclass
class TicketView:
    """One ticket collapsed to a single centroid vector + frontmatter."""
    ticket_id: str
    source_kind: str            # "tickets" | "landed"
    source_path: str
    status: str
    cluster: str | None
    title: str
    landed_on: str | None
    row_indices: list[int]      # rows in idx.vectors / idx.chunks
    centroid: np.ndarray        # unit-normalized


# ── entry point ─────────────────────────────────────────────────────────────

def main() -> int:
    args = _parse_args()

    if args.mode == "seed" and not args.seed:
        print("ERROR: --mode seed requires --seed <id|text>", file=sys.stderr)
        return 2

    try:
        idx = load_index(REPO_ROOT)
    except FileNotFoundError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2

    if idx.embedder != EMBEDDER_NAME:
        print(
            f"WARN: index built with {idx.embedder}, current embedder is "
            f"{EMBEDDER_NAME} — query embedding may be incompatible. "
            f"Run `just similar-build --full` to rebuild.",
            file=sys.stderr,
        )

    paths_now = discover_corpus_files(REPO_ROOT)
    stale = stale_files(REPO_ROOT, idx, paths_now)
    if stale:
        print(
            f"WARN: index stale ({len(stale)} files changed) — "
            f"run `just similar-build` to refresh",
            file=sys.stderr,
        )

    tickets = _build_ticket_views(idx)

    query_vec, query_sources, breakdown, primary_cluster = _build_query_vector(
        args, tickets, idx,
    )
    if query_vec is None:
        print(
            f"ERROR: --mode {args.mode} produced no query sources "
            f"(all source sets empty)",
            file=sys.stderr,
        )
        return 2

    statuses = {"ready", "blocked"} if args.include_blocked else {"ready"}
    excluded = {(s.source_kind, s.ticket_id) for s in query_sources}
    candidates = [
        t for t in tickets
        if t.source_kind == "tickets"
        and t.status in statuses
        and (t.source_kind, t.ticket_id) not in excluded
    ]
    if not candidates:
        print(
            f"ERROR: no candidate tickets with status in {sorted(statuses)}",
            file=sys.stderr,
        )
        return 1

    ranked = _rank(query_vec, candidates, primary_cluster)
    top = ranked[: args.top]

    source_rows = sorted({r for s in query_sources for r in s.row_indices})
    if not source_rows and args.mode == "substrate":
        # Substrate mode also uses the spec rows directly — fold them in
        # so rationales can point at the spec, not just at cluster tickets.
        source_rows = sorted([
            i for i, c in enumerate(idx.chunks)
            if c["source_path"] == SUBSTRATE_SPEC_PATH
        ])

    results = [_make_result(t, score, idx, source_rows) for t, score in top]

    env = Envelope(
        query={
            "mode": args.mode,
            "seed": args.seed,
            "top_k": args.top,
            "landed_window": args.landed_window,
            "include_blocked": args.include_blocked,
            "weights": (
                {"momentum": args.w_momentum,
                 "wip": args.w_wip,
                 "substrate": args.w_substrate}
                if args.mode == "blend" else None
            ),
            "embedder": idx.embedder,
        },
        scan_stats={
            "indexed_chunks": len(idx.chunks),
            "indexed_tickets": len(tickets),
            "candidates": len(candidates),
            "query_sources": len(query_sources),
            "returned": len(top),
            "narrow_by": ["mode", "top", "include-blocked", "landed-window"],
            "index_stale_files": len(stale),
            "mode_breakdown": breakdown,
        },
        results=results,
        narrative=_make_narrative(
            args.mode, breakdown, len(candidates), len(top),
        ),
        next=_suggest_next(args.mode, args.seed, args.top),
    )
    emit(env, fmt="text" if args.text else "json")
    return 0 if results else 1


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Embedding-based ready-ticket recommender.",
    )
    parser.add_argument(
        "--mode",
        default="blend",
        choices=["blend", "momentum", "wip", "substrate", "seed"],
        help="Query-vector mode (default: blend).",
    )
    parser.add_argument(
        "--seed",
        default=None,
        help=("Seed value when --mode seed: ticket id (bare number) or "
              "free text (quoted)."),
    )
    parser.add_argument(
        "--top", "-k", type=int, default=DEFAULT_TOP_K,
        help=f"Number of recommendations (default: {DEFAULT_TOP_K}).",
    )
    parser.add_argument(
        "--landed-window", type=int, default=DEFAULT_LANDED_WINDOW,
        help=("How many recent landings drive momentum "
              f"(default: {DEFAULT_LANDED_WINDOW})."),
    )
    parser.add_argument(
        "--include-blocked", action="store_true",
        help="Also surface tickets with status=blocked (default: ready only).",
    )
    parser.add_argument("--w-momentum", type=float,
                        default=DEFAULT_WEIGHTS["momentum"])
    parser.add_argument("--w-wip", type=float,
                        default=DEFAULT_WEIGHTS["wip"])
    parser.add_argument("--w-substrate", type=float,
                        default=DEFAULT_WEIGHTS["substrate"])
    parser.add_argument(
        "--text", action="store_true",
        help="Emit text envelope instead of JSON.",
    )
    return parser.parse_args()


# ── ticket views ────────────────────────────────────────────────────────────

def _build_ticket_views(idx: Index) -> list[TicketView]:
    """Group every ticket / landed chunk by (source_kind, ticket_id) and
    collapse to a single centroid + the heading frontmatter."""
    rows_by_key: dict[tuple[str, str], list[int]] = {}
    meta_by_key: dict[tuple[str, str], dict[str, Any]] = {}
    path_by_key: dict[tuple[str, str], str] = {}
    for i, c in enumerate(idx.chunks):
        if c["source_kind"] not in ("tickets", "landed"):
            continue
        md = c.get("metadata", {}) or {}
        ticket_id = str(md.get("ticket_id") or "").strip()
        if not ticket_id:
            continue
        key = (c["source_kind"], ticket_id)
        rows_by_key.setdefault(key, []).append(i)
        meta_by_key.setdefault(key, md)
        path_by_key.setdefault(key, c["source_path"])

    out: list[TicketView] = []
    for key, rows in rows_by_key.items():
        source_kind, ticket_id = key
        md = meta_by_key[key]
        sub = idx.vectors[np.array(rows, dtype=np.int64)]
        mean = sub.mean(axis=0)
        norm = float(np.linalg.norm(mean))
        centroid = (mean / norm) if norm else mean
        cluster_raw = md.get("cluster")
        cluster = (
            cluster_raw
            if cluster_raw not in ("—", None, "", "null")
            else None
        )
        out.append(TicketView(
            ticket_id=ticket_id,
            source_kind=source_kind,
            source_path=path_by_key[key],
            status=str(md.get("status") or "?"),
            cluster=cluster,
            title=str(md.get("title") or ""),
            landed_on=md.get("landed_on") or None,
            row_indices=rows,
            centroid=centroid.astype(np.float32, copy=False),
        ))
    return out


# ── query-vector composition ────────────────────────────────────────────────

def _build_query_vector(
    args: argparse.Namespace,
    tickets: list[TicketView],
    idx: Index,
) -> tuple[np.ndarray | None, list[TicketView], dict[str, int], str | None]:
    """Build the query vector + the list of source TicketViews used for
    rationale matching. Also returns the cluster of the highest-weighted
    component (used as a tiebreak hint). Returns (None, ...) if every
    component is empty."""
    breakdown: dict[str, int] = {}

    if args.mode == "seed":
        return _seed_vector(args.seed, tickets, breakdown)

    components: list[tuple[str, np.ndarray, float, list[TicketView]]] = []

    if args.mode in ("blend", "momentum"):
        comp = _momentum_component(tickets, args.landed_window)
        if comp is not None:
            vec, sources = comp
            components.append(("momentum", vec, args.w_momentum, sources))
            breakdown["momentum"] = len(sources)

    if args.mode in ("blend", "wip"):
        comp = _wip_component(tickets)
        if comp is not None:
            vec, sources = comp
            components.append(("wip", vec, args.w_wip, sources))
            breakdown["wip"] = len(sources)

    if args.mode in ("blend", "substrate"):
        comp = _substrate_component(tickets, idx)
        if comp is not None:
            vec, sources = comp
            components.append(("substrate", vec, args.w_substrate, sources))
            breakdown["substrate"] = len(sources)

    if not components:
        return None, [], breakdown, None

    if args.mode != "blend" and len(components) == 1:
        _, vec, _, sources = components[0]
        primary_cluster = _dominant_cluster(sources)
        return vec, sources, breakdown, primary_cluster

    weights = np.array([w for _, _, w, _ in components], dtype=np.float32)
    if weights.sum() <= 0:
        return None, [], breakdown, None
    weights = weights / weights.sum()
    stacked = np.stack([v for _, v, _, _ in components])
    blended = (weights.reshape(-1, 1) * stacked).sum(axis=0)
    norm = float(np.linalg.norm(blended))
    if norm:
        blended = blended / norm

    all_sources: list[TicketView] = []
    seen: set[tuple[str, str]] = set()
    for _, _, _, sources in components:
        for s in sources:
            sig = (s.source_kind, s.ticket_id)
            if sig in seen:
                continue
            seen.add(sig)
            all_sources.append(s)

    # Primary cluster = the cluster of the highest-weighted *non-empty*
    # component, used only for near-tie tiebreaking.
    heaviest = max(components, key=lambda c: c[2])
    primary_cluster = _dominant_cluster(heaviest[3])

    return blended, all_sources, breakdown, primary_cluster


def _momentum_component(
    tickets: list[TicketView], window: int,
) -> tuple[np.ndarray, list[TicketView]] | None:
    landed = [t for t in tickets if t.source_kind == "landed" and t.landed_on]
    if not landed:
        return None
    landed.sort(
        key=lambda t: (
            t.landed_on or "",
            int(t.ticket_id) if t.ticket_id.isdigit() else 0,
        ),
        reverse=True,
    )
    sources = landed[: max(1, window)]
    return _average_centroids(sources), sources


def _wip_component(
    tickets: list[TicketView],
) -> tuple[np.ndarray, list[TicketView]] | None:
    sources = [
        t for t in tickets
        if t.source_kind == "tickets" and t.status == "in-progress"
    ]
    if not sources:
        return None
    return _average_centroids(sources), sources


def _substrate_component(
    tickets: list[TicketView], idx: Index,
) -> tuple[np.ndarray, list[TicketView]] | None:
    spec_rows = [
        i for i, c in enumerate(idx.chunks)
        if c["source_path"] == SUBSTRATE_SPEC_PATH
    ]
    cluster_sources = [
        t for t in tickets
        if t.source_kind == "tickets"
        and t.cluster in SUBSTRATE_CLUSTERS
        and t.status not in ("done", "dropped")
    ]
    if not spec_rows and not cluster_sources:
        return None

    centroids: list[np.ndarray] = []
    if spec_rows:
        centroids.append(_centroid_from_rows(idx, spec_rows))
    for t in cluster_sources:
        centroids.append(t.centroid)
    arr = np.stack(centroids)
    mean = arr.mean(axis=0)
    norm = float(np.linalg.norm(mean))
    if norm:
        mean = mean / norm
    # Sources used for rationale matching = cluster tickets only; the
    # spec rows are folded into source_rows separately when --mode
    # substrate is exclusive (handled in main()).
    return mean, cluster_sources


def _seed_vector(
    seed: str | None,
    tickets: list[TicketView],
    breakdown: dict[str, int],
) -> tuple[np.ndarray | None, list[TicketView], dict[str, int], str | None]:
    if not seed:
        return None, [], breakdown, None
    if seed.isdigit():
        sources = [t for t in tickets if t.ticket_id == seed]
        if sources:
            vec = _average_centroids(sources)
            breakdown["seed_ticket"] = len(sources)
            return vec, sources, breakdown, _dominant_cluster(sources)
        breakdown["seed_text_fallback"] = 1
    arr = embed_batch([seed])
    breakdown["seed_text"] = 1
    vec = arr[0].astype(np.float32, copy=False)
    norm = float(np.linalg.norm(vec))
    if norm:
        vec = vec / norm
    return vec, [], breakdown, None


def _average_centroids(sources: list[TicketView]) -> np.ndarray:
    arr = np.stack([t.centroid for t in sources])
    mean = arr.mean(axis=0)
    norm = float(np.linalg.norm(mean))
    return (mean / norm) if norm else mean


def _centroid_from_rows(idx: Index, rows: list[int]) -> np.ndarray:
    sub = idx.vectors[np.array(rows, dtype=np.int64)]
    mean = sub.mean(axis=0)
    norm = float(np.linalg.norm(mean))
    return (mean / norm) if norm else mean


def _dominant_cluster(sources: list[TicketView]) -> str | None:
    counts: dict[str, int] = {}
    for s in sources:
        if s.cluster:
            counts[s.cluster] = counts.get(s.cluster, 0) + 1
    if not counts:
        return None
    return max(counts.items(), key=lambda kv: kv[1])[0]


# ── ranking ─────────────────────────────────────────────────────────────────

def _rank(
    query_vec: np.ndarray,
    candidates: list[TicketView],
    primary_cluster: str | None,
) -> list[tuple[TicketView, float]]:
    if not candidates:
        return []
    mat = np.stack([t.centroid for t in candidates])
    sims = mat @ query_vec.astype(np.float32, copy=False)

    # Tiebreak only nudges within NEAR_TIE_DELTA: candidates matching
    # primary_cluster get a tiny lift so they sort above unrelated
    # near-ties. Nothing else changes.
    if primary_cluster:
        bonus = np.array(
            [NEAR_TIE_DELTA * 0.5 if t.cluster == primary_cluster else 0.0
             for t in candidates],
            dtype=np.float32,
        )
        sims = sims + bonus

    order = np.argsort(-sims)
    return [(candidates[int(i)], float(sims[int(i)])) for i in order]


# ── result + narrative shaping ──────────────────────────────────────────────

def _make_result(
    t: TicketView, score: float, idx: Index, source_rows: list[int],
) -> dict[str, Any]:
    rationale = _best_source_match(t.centroid, idx, source_rows)
    return {
        "id": t.ticket_id,
        "score": round(score, 4),
        "path": t.source_path,
        "title": t.title,
        "cluster": t.cluster,
        "status": t.status,
        "rationale": rationale,
        "summary": _summary_line(t, score, rationale),
    }


def _best_source_match(
    cand_centroid: np.ndarray,
    idx: Index,
    source_rows: list[int],
) -> dict[str, Any] | None:
    if not source_rows:
        return None
    rows = np.array(source_rows, dtype=np.int64)
    sub = idx.vectors[rows]
    sims = sub @ cand_centroid.astype(np.float32, copy=False)
    j = int(np.argmax(sims))
    chunk = idx.chunks[int(rows[j])]
    md = chunk.get("metadata", {}) or {}
    return {
        "score": round(float(sims[j]), 4),
        "path": chunk["source_path"],
        "section": chunk.get("section"),
        "ticket_id": md.get("ticket_id"),
        "source_kind": chunk["source_kind"],
    }


def _summary_line(
    t: TicketView, score: float, rationale: dict[str, Any] | None,
) -> str:
    head = (
        f"[{t.ticket_id}] {t.title}".strip()
        + f" (cluster: {t.cluster or '—'})"
    )
    if not rationale:
        return f"{head}\n        {score:.2f}"
    section = rationale.get("section") or ""
    sk = rationale.get("source_kind", "")
    rid = rationale.get("ticket_id") or Path(rationale.get("path", "")).stem
    section_part = f" § {section}" if section and section != "_full" else ""
    return (
        f"{head}\n"
        f"        {score:.2f} · adjacent to {sk}/{rid}{section_part}"
    )


def _make_narrative(
    mode: str, breakdown: dict[str, int],
    n_candidates: int, n_returned: int,
) -> str:
    if not n_returned:
        return f"no recommendations for mode `{mode}`."
    bd = ", ".join(f"{n} {k}" for k, n in breakdown.items()) or "no sources"
    return (
        f"mode `{mode}` over {bd} → "
        f"{n_candidates} candidates → top {n_returned}."
    )


def _suggest_next(mode: str, seed: str | None, k: int) -> list[str]:
    out: list[str] = []
    if mode == "blend":
        out.append("just next --mode momentum")
        out.append("just next --mode wip")
        out.append("just next --mode substrate")
        out.append("just next --mode seed --seed <ticket-id>")
    elif mode == "momentum":
        out.append("just next  # blend with wip + substrate")
        out.append(f"just next --mode momentum --landed-window {max(1, k * 2)}")
    elif mode in ("wip", "substrate"):
        out.append("just next  # blend in landed momentum")
    elif mode == "seed" and seed:
        out.append(f"just similar {seed!r}  # raw retrieval over all corpora")
        out.append("just next  # default blended mode")
    return out


if __name__ == "__main__":
    raise SystemExit(main())
