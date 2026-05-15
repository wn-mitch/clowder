#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "fastembed>=0.4",
#     "numpy>=1.26",
# ]
# ///
"""`just similar` — semantic retrieval over Clowder prose.

Three input shapes accepted, auto-detected:

  just similar 189                          # ticket id (bare number)
  just similar tickets/175.md               # repo-relative file path
  just similar "starvation cluster"         # free text

For ticket-id and file-path queries the query vector is the *centroid*
of that source's existing chunks. The source's own chunks are excluded
from results (no self-matches). For free-text queries the text is
embedded directly.

Emits the standard envelope (see scripts/logq/envelope.py): query echo,
scan stats, top-K results with stable ids, narrative gloss, and
suggested next commands.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

import numpy as np

# Make sibling modules importable regardless of how this is invoked.
sys.path.insert(0, str(Path(__file__).resolve().parent))
# Make scripts/logq/envelope.py importable.
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "logq"))

from chunkers import (                                                    # noqa: E402
    chunk_path, chunker_for, discover_corpus_files,
)
from embed import EMBEDDER_NAME, embed_batch                              # noqa: E402
from envelope import Envelope, emit                                       # noqa: E402  type: ignore[import-not-found]
from retrieve import (                                                    # noqa: E402
    Index, chunks_by_ticket_id, load_index, stale_files, top_k,
    weighted_centroid_from_rows,
)


REPO_ROOT = Path(__file__).resolve().parents[2]

ALL_CORPUSES = ["tickets", "landed", "balance", "pre-existing", "systems",
                "dses", "planner", "markers"]


def _build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Semantic retrieval over Clowder prose.",
    )
    parser.add_argument(
        "input",
        nargs="+",
        help=("Ticket id (bare number), repo-relative file path, or free-text "
              "query. Free-text queries may be passed as multiple tokens — "
              "they are joined with a space before classification — so the "
              "justfile `{{ARGS}}` passthrough preserves multi-word intent "
              "without requiring shell-quoting that just doesn't preserve."),
    )
    parser.add_argument(
        "--top-k", "-k",
        type=int, default=5,
        help="Number of results to return (default: 5).",
    )
    parser.add_argument(
        "--corpus",
        default=None,
        help=("Comma-separated corpus filter. Default: all. Options: "
              + ", ".join(ALL_CORPUSES)),
    )
    parser.add_argument(
        "--text", action="store_true",
        help="Emit text envelope instead of JSON.",
    )
    parser.add_argument(
        "--rebuild", action="store_true",
        help="Force a full index rebuild before running the query.",
    )
    return parser


def main() -> int:
    args = _build_arg_parser().parse_args()
    input_str = " ".join(args.input)

    if args.rebuild:
        _rebuild_index()

    try:
        idx = load_index(REPO_ROOT)
    except FileNotFoundError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2

    if idx.embedder != EMBEDDER_NAME:
        print(
            f"WARN: index built with {idx.embedder}, current embedder is "
            f"{EMBEDDER_NAME} — query embedding will be incompatible. "
            f"Run `just similar-build --full` to rebuild.",
            file=sys.stderr,
        )

    corpus_filter = _parse_corpus_filter(args.corpus)

    # Resolve input to (query_vec, input_kind, exclude_chunk_ids).
    try:
        query_vec, input_kind, exclude_ids, resolution_note = _resolve_query(
            input_str, idx,
        )
    except ValueError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2

    # Stale-files warning to stderr — non-blocking.
    paths_now = discover_corpus_files(REPO_ROOT)
    stale = stale_files(REPO_ROOT, idx, paths_now)
    if stale:
        print(
            f"WARN: index stale ({len(stale)} files changed) — "
            f"run `just similar-build` to refresh",
            file=sys.stderr,
        )

    hits = top_k(
        idx, query_vec, args.top_k,
        corpus_filter=corpus_filter,
        exclude_chunk_ids=exclude_ids,
    )

    results = [_chunk_to_result(idx.chunks[i], score) for i, score in hits]

    narrative = _make_narrative(input_str, input_kind, results, resolution_note)

    suggestions = _suggest_next(input_str, input_kind, args.top_k, corpus_filter, len(hits))

    env = Envelope(
        query={
            "input": input_str,
            "input_kind": input_kind,
            "top_k": args.top_k,
            "corpus": sorted(corpus_filter) if corpus_filter else ALL_CORPUSES,
            "embedder": idx.embedder,
        },
        scan_stats={
            "scanned": len(idx.chunks),
            "returned": len(results),
            "more_available": len(hits) == args.top_k and args.top_k < len(idx.chunks),
            "narrow_by": ["corpus", "top_k"],
            "index_stale_files": len(stale),
        },
        results=results,
        narrative=narrative,
        next=suggestions,
    )
    emit(env, fmt="text" if args.text else "json")
    return 0 if results else 1


# ── input resolution ────────────────────────────────────────────────────────

def _resolve_query(
    raw: str,
    idx: Index,
) -> tuple[np.ndarray, str, set[str], str | None]:
    """Decide the input kind and compute the query vector.

    Returns `(query_vec, input_kind, exclude_chunk_ids, resolution_note)`.
    `resolution_note` is a one-liner included in the narrative when the
    resolution did something non-obvious (e.g. fell back to free-text
    because no chunks matched the ticket id)."""
    # Bare integer → ticket id.
    if raw.isdigit():
        rows = chunks_by_ticket_id(idx, raw)
        if rows:
            centroid = weighted_centroid_from_rows(idx, rows)
            exclude = {idx.chunks[r]["chunk_id"] for r in rows}
            return centroid, "ticket_id", exclude, None
        # Fall through to free-text — maybe the ticket isn't indexed yet.
        return _embed_free_text(raw), "free_text", set(), (
            f"no chunks found for ticket id {raw} — fell back to free-text query"
        )

    # Repo-relative path that exists.
    candidate = REPO_ROOT / raw
    if candidate.exists() and candidate.is_file():
        return _resolve_file_path(candidate, idx, raw)

    # Bare filename like `175.md` or `175-foo.md` — try resolving via
    # tickets/landed by ticket-id prefix in the filename.
    if raw.endswith(".md"):
        m = _match_ticket_filename(raw)
        if m:
            rows = chunks_by_ticket_id(idx, m)
            if rows:
                centroid = weighted_centroid_from_rows(idx, rows)
                exclude = {idx.chunks[r]["chunk_id"] for r in rows}
                return centroid, "ticket_id", exclude, (
                    f"resolved filename `{raw}` → ticket {m}"
                )

    # Anything else: free text.
    return _embed_free_text(raw), "free_text", set(), None


def _resolve_file_path(
    path: Path,
    idx: Index,
    raw: str,
) -> tuple[np.ndarray, str, set[str], str | None]:
    """File path resolution: prefer existing index rows for that file;
    if none (file isn't in any indexed corpus, or hasn't been embedded
    yet), chunk + embed it ad-hoc as the query."""
    rel = str(path.relative_to(REPO_ROOT))
    matching_rows = [
        i for i, c in enumerate(idx.chunks) if c["source_path"] == rel
    ]
    if matching_rows:
        centroid = weighted_centroid_from_rows(idx, matching_rows)
        exclude = {idx.chunks[r]["chunk_id"] for r in matching_rows}
        return centroid, "file_path", exclude, None

    if chunker_for(rel) is None:
        return _embed_free_text(path.read_text(encoding="utf-8")[:4000]), \
            "file_path", set(), (
                f"`{rel}` is outside the indexed corpus — "
                f"embedded its first 4k chars as a free-text query"
            )

    # File is in the corpus but missing from the index — chunk + embed
    # ad-hoc (don't mutate the index here; that's index.py's job).
    chunks_now = chunk_path(path, REPO_ROOT)
    if not chunks_now:
        return _embed_free_text(path.read_text(encoding="utf-8")[:4000]), \
            "file_path", set(), (
                f"`{rel}` produced no chunks — embedded raw content"
            )
    vecs = embed_batch([c.text for c in chunks_now])
    centroid = vecs.mean(axis=0)
    centroid /= np.linalg.norm(centroid) or 1.0
    exclude: set[str] = set()  # nothing to exclude — file isn't in index.
    return centroid, "file_path", exclude, (
        f"`{rel}` not yet indexed — embedded ad-hoc; "
        f"run `just similar-build` to persist"
    )


_TICKET_FILENAME_RE = __import__("re").compile(r"^(\d+)(?:[-.].+)?$")


def _match_ticket_filename(name: str) -> str | None:
    """Pull a ticket id from a filename like `189.md` or
    `189-schedule-edge.md`."""
    stem = Path(name).stem
    m = _TICKET_FILENAME_RE.match(stem)
    return m.group(1) if m else None


def _embed_free_text(text: str) -> np.ndarray:
    """Embed a single free-text string and return a 1-D unit vector."""
    arr = embed_batch([text])
    return arr[0]


# ── result + narrative shaping ──────────────────────────────────────────────

def _chunk_to_result(chunk: dict[str, Any], score: float) -> dict[str, Any]:
    """Render an index chunk as an envelope `results` entry. The
    `summary` field is plucked from chunk metadata when available
    so the text envelope renders something readable."""
    md = chunk.get("metadata", {}) or {}
    section = chunk.get("section")
    summary_bits = []
    if md.get("title"):
        summary_bits.append(md["title"])
    elif md.get("doc_title"):
        summary_bits.append(md["doc_title"])
    elif md.get("item_name"):
        summary_bits.append(f"{md.get('item_kind', 'item')} {md['item_name']}")
    if section and section != "_full":
        summary_bits.append(f"§ {section}")
    summary = " — ".join(summary_bits) or chunk["chunk_id"]

    return {
        "id": chunk["chunk_id"],
        "score": round(score, 4),
        "path": chunk["source_path"],
        "section": section,
        "source_kind": chunk["source_kind"],
        "metadata": _slim_metadata(md),
        "summary": summary,
    }


def _slim_metadata(md: dict[str, Any]) -> dict[str, Any]:
    """Return only the keys that aid disambiguation in the envelope."""
    keep = ("ticket_id", "title", "status", "cluster", "landed_on",
            "doc_title", "item_kind", "item_name")
    return {k: v for k, v in md.items() if k in keep and v is not None}


def _make_narrative(
    raw_input: str,
    input_kind: str,
    results: list[dict[str, Any]],
    resolution_note: str | None,
) -> str:
    if not results:
        return (f"no results for {input_kind} `{raw_input}`. Try widening "
                f"--top-k or removing --corpus.")
    by_kind: dict[str, int] = {}
    paths: set[str] = set()
    for r in results:
        by_kind[r["source_kind"]] = by_kind.get(r["source_kind"], 0) + 1
        paths.add(r["path"])
    breakdown = ", ".join(f"{n} {k}" for k, n in by_kind.items())
    bits = [
        f"{len(results)} chunks across {len(paths)} documents ({breakdown})",
    ]
    if resolution_note:
        bits.append(resolution_note)
    return ". ".join(bits) + "."


def _suggest_next(
    raw: str,
    input_kind: str,
    k: int,
    corpus_filter: set[str] | None,
    n_results: int,
) -> list[str]:
    out: list[str] = []
    if input_kind == "ticket_id":
        if not corpus_filter or "landed" not in corpus_filter:
            out.append(f"just similar {raw} --corpus landed")
        out.append(f"just similar {raw} --corpus dses,planner,markers")
    elif input_kind == "file_path":
        out.append(f"just similar {raw} --top-k {k * 2}")
    else:
        out.append(f"just similar {raw!r} --corpus tickets,landed")
        out.append(f"just similar {raw!r} --corpus balance,systems")
    if n_results == k:
        out.append(f"just similar {raw if not ' ' in raw else repr(raw)} --top-k {k * 2}")
    return out


# ── helpers ─────────────────────────────────────────────────────────────────

def _parse_corpus_filter(raw: str | None) -> set[str] | None:
    if raw is None:
        return None
    parts = [p.strip() for p in raw.split(",") if p.strip()]
    unknown = [p for p in parts if p not in ALL_CORPUSES]
    if unknown:
        raise SystemExit(
            f"unknown corpus filter(s): {unknown}. "
            f"Valid: {', '.join(ALL_CORPUSES)}"
        )
    return set(parts)


def _rebuild_index() -> None:
    """Invoke index.py in-process for a full rebuild."""
    import subprocess
    subprocess.check_call(
        [sys.executable, str(Path(__file__).parent / "index.py"), "--full"],
    )


if __name__ == "__main__":
    raise SystemExit(main())
