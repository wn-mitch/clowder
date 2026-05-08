#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "fastembed>=0.4",
#     "numpy>=1.26",
# ]
# ///
"""Build the embedding index for `just similar`.

Usage:
    just similar-build                # incremental — re-embed changed files only
    just similar-build --full         # full rebuild
    just similar-build --dry-run      # report what would be re-embedded, don't run

Walks the corpus prefixes declared in `chunkers.py::_DISPATCH`,
chunks each file, embeds new/changed chunks, and writes
`logs/.embeddings/{index.npz, index.meta.json}`.

Incremental rebuild logic: per-file mtime is stored in the index
metadata. A file whose on-disk mtime exceeds the recorded mtime is
re-chunked + re-embedded; deleted files have their chunks dropped;
unchanged files keep their existing embeddings (no re-embed).
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

import numpy as np

# Sibling-module imports — make this script runnable as
# `python scripts/similar/index.py` (direct) or via `uv run`.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from chunkers import Chunk, chunk_path, discover_corpus_files       # noqa: E402
from embed import EMBEDDER_NAME, embed_batch, embedding_dim         # noqa: E402
from retrieve import INDEX_DIR, Index, load_index, save_index       # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Build the just similar embedding index.",
    )
    parser.add_argument(
        "--full",
        action="store_true",
        help="Discard the existing index and rebuild from scratch.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report what would be re-embedded; don't write.",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress progress output.",
    )
    args = parser.parse_args()

    log = (lambda *a, **kw: None) if args.quiet else _log

    paths = discover_corpus_files(REPO_ROOT)
    log(f"discovered {len(paths)} corpus files")

    existing: Index | None = None
    if not args.full:
        try:
            existing = load_index(REPO_ROOT)
            log(f"loaded existing index: {len(existing.chunks)} chunks "
                f"(embedder: {existing.embedder})")
            if existing.embedder != EMBEDDER_NAME:
                log(f"  embedder mismatch ({existing.embedder} vs {EMBEDDER_NAME}) "
                    f"— forcing full rebuild")
                existing = None
        except FileNotFoundError:
            log("no existing index — running full build")

    keep_chunks: list[dict] = []
    keep_vectors_rows: list[int] = []
    changed_paths: list[Path] = []

    if existing is not None:
        # Decide which chunks to keep (file unchanged) vs re-embed (changed
        # or new). Map each path → list of existing rows that came from it.
        rows_by_path: dict[str, list[int]] = {}
        for row, c in enumerate(existing.chunks):
            rows_by_path.setdefault(c["source_path"], []).append(row)

        current_set = {str(p.relative_to(REPO_ROOT)) for p in paths}

        for p in paths:
            rel = str(p.relative_to(REPO_ROOT))
            actual_mtime = p.stat().st_mtime
            recorded = existing.source_mtimes.get(rel)
            if recorded is not None and abs(actual_mtime - recorded) <= 1e-6:
                # Unchanged — keep the existing rows for this file.
                for row in rows_by_path.get(rel, []):
                    keep_vectors_rows.append(row)
                    keep_chunks.append(existing.chunks[row])
            else:
                changed_paths.append(p)

        # Files that vanished from the corpus get their rows dropped (we
        # never added them to keep_vectors_rows above).
        vanished = set(existing.source_mtimes.keys()) - current_set
        if vanished:
            log(f"dropping {len(vanished)} vanished sources: {sorted(vanished)[:3]}{'...' if len(vanished) > 3 else ''}")
    else:
        changed_paths = list(paths)

    log(f"will re-embed {len(changed_paths)} files "
        f"(keeping {len(keep_chunks)} unchanged chunks)")

    if args.dry_run:
        for p in changed_paths[:20]:
            log(f"  would re-embed: {p.relative_to(REPO_ROOT)}")
        if len(changed_paths) > 20:
            log(f"  ... and {len(changed_paths) - 20} more")
        return 0

    # Chunk the changed files.
    new_chunks: list[Chunk] = []
    for p in changed_paths:
        new_chunks.extend(chunk_path(p, REPO_ROOT))
    log(f"chunked {len(new_chunks)} new chunks across {len(changed_paths)} files")

    # Embed.
    if new_chunks:
        log(f"embedding {len(new_chunks)} chunks via {EMBEDDER_NAME}...")
        t0 = time.monotonic()
        new_vectors = embed_batch([c.text for c in new_chunks])
        log(f"  embedded in {time.monotonic() - t0:.1f}s")
    else:
        new_vectors = np.zeros((0, embedding_dim()), dtype=np.float32)

    # Stitch the kept rows + new rows together.
    if existing is not None and keep_vectors_rows:
        kept_vectors = existing.vectors[np.array(keep_vectors_rows, dtype=np.int64)]
        all_vectors = np.vstack([kept_vectors, new_vectors]) if new_vectors.size else kept_vectors
    else:
        all_vectors = new_vectors

    all_chunks = list(keep_chunks) + [c.to_dict() for c in new_chunks]

    # Build source_mtimes from the *current* paths (drops vanished entries
    # automatically — we only record what's still on disk).
    source_mtimes = {
        str(p.relative_to(REPO_ROOT)): p.stat().st_mtime for p in paths
    }

    idx = Index(
        vectors=all_vectors,
        chunks=all_chunks,
        embedder=EMBEDDER_NAME,
        dim=embedding_dim(),
        source_mtimes=source_mtimes,
        built_at=time.time(),
    )

    save_index(REPO_ROOT, idx)
    log(f"wrote {len(all_chunks)} chunks ({all_vectors.shape}) to {INDEX_DIR}/")
    return 0


def _log(*args, **kwargs) -> None:
    print(*args, **kwargs, file=sys.stderr, flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
