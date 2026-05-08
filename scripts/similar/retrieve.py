"""Retrieval over a built embedding index.

Index layout on disk (`logs/.embeddings/`):
- `index.npz`           — single npz with `vectors: float32 (N, D)`
                          and `chunk_ids: object array of length N`.
- `index.meta.json`     — `{ embedder, dim, source_mtimes: {path: mtime},
                            chunks: [ ... per-chunk metadata ... ] }`.

`chunks` is parallel to `vectors` row-by-row — index `i` of vectors
matches index `i` of chunks.

Cosine similarity is implemented as a plain dot product because the
embedder normalizes outputs to unit L2 norm. At ~3.7k chunks the
brute-force scan is sub-100ms; an ANN library would be premature.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np


INDEX_DIR = Path("logs/.embeddings")
INDEX_NPZ = INDEX_DIR / "index.npz"
INDEX_META = INDEX_DIR / "index.meta.json"


@dataclass
class Index:
    vectors: np.ndarray              # (N, dim) float32, L2-normalized
    chunks: list[dict[str, Any]]     # parallel to vectors
    embedder: str
    dim: int
    source_mtimes: dict[str, float]  # repo-relative path → mtime when embedded
    built_at: float                  # unix ts of last full-or-partial build


def load_index(repo_root: Path) -> Index:
    """Load the index from disk. Raises `FileNotFoundError` with a
    user-actionable message if the index hasn't been built."""
    npz_path = repo_root / INDEX_NPZ
    meta_path = repo_root / INDEX_META
    if not npz_path.exists() or not meta_path.exists():
        raise FileNotFoundError(
            f"index not found at {INDEX_DIR}/ — run `just similar-build` first"
        )
    npz = np.load(npz_path, allow_pickle=True)
    vectors = npz["vectors"].astype(np.float32, copy=False)
    meta = json.loads(meta_path.read_text(encoding="utf-8"))
    chunks = meta["chunks"]
    if vectors.shape[0] != len(chunks):
        raise RuntimeError(
            f"index corruption: {vectors.shape[0]} vectors vs "
            f"{len(chunks)} chunk records"
        )
    return Index(
        vectors=vectors,
        chunks=chunks,
        embedder=meta.get("embedder", "?"),
        dim=int(meta.get("dim", vectors.shape[1])),
        source_mtimes=meta.get("source_mtimes", {}),
        built_at=float(meta.get("built_at", 0.0)),
    )


def save_index(repo_root: Path, idx: Index) -> None:
    """Persist the index to disk atomically.

    Writes to `.tmp` siblings and renames into place so a crash
    mid-write leaves either the previous valid index or the new
    valid index — never a torn pair where vectors and chunks
    disagree on row count. Caller is responsible for ensuring the
    directory exists."""
    npz_path = repo_root / INDEX_NPZ
    meta_path = repo_root / INDEX_META
    npz_path.parent.mkdir(parents=True, exist_ok=True)
    # `np.savez` auto-appends `.npz` to a filename that doesn't end
    # with it, so use an open file handle to bypass that heuristic
    # and keep our atomic-rename semantics intact.
    npz_tmp = npz_path.with_suffix(npz_path.suffix + ".tmp")
    meta_tmp = meta_path.with_suffix(meta_path.suffix + ".tmp")
    with open(npz_tmp, "wb") as f:
        np.savez(f, vectors=idx.vectors)
    meta = {
        "embedder": idx.embedder,
        "dim": idx.dim,
        "built_at": idx.built_at,
        "source_mtimes": idx.source_mtimes,
        "chunks": idx.chunks,
    }
    meta_tmp.write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    # Rename is atomic on POSIX. If we crash between these two renames,
    # the npz and meta would briefly disagree — but the next load_index
    # call would catch the row-count mismatch and surface a clear error
    # rather than silently using stale data.
    npz_tmp.replace(npz_path)
    meta_tmp.replace(meta_path)


def stale_files(repo_root: Path, idx: Index, current_paths: list[Path]) -> list[str]:
    """Return repo-relative paths whose on-disk mtime differs from the
    index's recorded mtime, plus any new paths not in the index. The
    query path uses this to surface a `WARN: index stale` line to
    stderr without auto-rebuilding."""
    stale: list[str] = []
    current_set = {str(p.relative_to(repo_root)) for p in current_paths}
    for rel in current_set:
        actual = (repo_root / rel).stat().st_mtime
        recorded = idx.source_mtimes.get(rel)
        if recorded is None or abs(actual - recorded) > 1e-6:
            stale.append(rel)
    for rel in idx.source_mtimes:
        if rel not in current_set:
            stale.append(rel)  # source disappeared
    return sorted(set(stale))


def top_k(
    idx: Index,
    query_vec: np.ndarray,
    k: int,
    *,
    corpus_filter: set[str] | None = None,
    exclude_chunk_ids: set[str] | None = None,
) -> list[tuple[int, float]]:
    """Return the top-K (chunk-row, similarity) pairs.

    `corpus_filter` accepts a set of `source_kind` values
    (e.g. {"tickets", "landed"}); pass `None` to retrieve across
    everything. `exclude_chunk_ids` is used to drop self-matches when
    the query is itself a known chunk-id (e.g. `just similar 189`
    shouldn't return ticket 189's own Why as the top hit).

    Implementation: cosine = dot product (vectors are unit-norm).
    `np.argpartition` gets top-K in O(N) without sorting the rest;
    we then sort the K-element slice descending."""
    if idx.vectors.shape[0] == 0:
        return []
    sims = idx.vectors @ query_vec.astype(np.float32, copy=False)

    if corpus_filter is not None or exclude_chunk_ids:
        mask = np.ones(len(sims), dtype=bool)
        if corpus_filter is not None:
            for i, c in enumerate(idx.chunks):
                if c.get("source_kind") not in corpus_filter:
                    mask[i] = False
        if exclude_chunk_ids:
            for i, c in enumerate(idx.chunks):
                if c.get("chunk_id") in exclude_chunk_ids:
                    mask[i] = False
        sims = np.where(mask, sims, -np.inf)

    n = sims.shape[0]
    k = min(k, n)
    if k <= 0:
        return []
    # argpartition with `-k` puts the top-K (un-sorted) at the end.
    part_idx = np.argpartition(sims, -k)[-k:]
    # Now sort just those K by similarity descending.
    sorted_part = part_idx[np.argsort(-sims[part_idx])]
    return [(int(i), float(sims[i])) for i in sorted_part if sims[i] > -np.inf]


def chunks_by_ticket_id(idx: Index, ticket_id: int | str) -> list[int]:
    """Return chunk-row indices belonging to a given ticket id (across
    both tickets/ and landed/). Used when the user passes a bare
    ticket number — the query vector is the *centroid* of that
    ticket's section vectors, so retrieval has a stable concept of
    'similar to ticket 189' even though the ticket has many chunks."""
    target = str(ticket_id)
    out = []
    for i, c in enumerate(idx.chunks):
        if str(c.get("metadata", {}).get("ticket_id")) == target:
            out.append(i)
    return out
