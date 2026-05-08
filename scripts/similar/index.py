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
    parser.add_argument(
        "--checkpoint-every",
        type=int, default=10,
        help="Flush the index after every N files. Lower = safer "
             "(less re-work on crash) but more disk I/O.",
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

    # Build the seed in-memory state from the kept (unchanged-file) rows.
    # We will append new files' chunks + vectors to these as each file
    # finishes embedding, and flush the index every CHECKPOINT_EVERY files.
    # That makes the build resumable: a kill mid-run leaves a valid
    # partial index whose source_mtimes only includes completed files,
    # so the next invocation re-embeds only the remainder.
    if existing is not None and keep_vectors_rows:
        all_vectors = existing.vectors[np.array(keep_vectors_rows, dtype=np.int64)]
    else:
        all_vectors = np.zeros((0, embedding_dim()), dtype=np.float32)
    all_chunks: list[dict] = list(keep_chunks)
    source_mtimes: dict[str, float] = {
        c["source_path"]: existing.source_mtimes.get(c["source_path"], 0.0)
        for c in all_chunks
    } if existing is not None else {}

    log(f"embedding {len(changed_paths)} files via {EMBEDDER_NAME} "
        f"(checkpoint every {args.checkpoint_every} files)...")
    t0 = time.monotonic()
    files_done_since_flush = 0
    chunks_emitted = 0

    for i, path in enumerate(changed_paths, start=1):
        rel = str(path.relative_to(REPO_ROOT))
        file_chunks = chunk_path(path, REPO_ROOT)
        if not file_chunks:
            # File is in the corpus but produced no chunks (e.g. empty
            # frontmatter-only ticket). Still record its mtime so the
            # next invocation skips it instead of re-trying every time.
            source_mtimes[rel] = path.stat().st_mtime
        else:
            file_vectors = embed_batch(
                [c.text for c in file_chunks],
                progress=not args.quiet,
            )
            if all_vectors.size == 0:
                all_vectors = file_vectors
            else:
                all_vectors = np.vstack([all_vectors, file_vectors])
            all_chunks.extend(c.to_dict() for c in file_chunks)
            source_mtimes[rel] = path.stat().st_mtime
            chunks_emitted += len(file_chunks)

        files_done_since_flush += 1
        # Flush periodically so a crash leaves a valid resume point.
        if files_done_since_flush >= args.checkpoint_every or i == len(changed_paths):
            idx = Index(
                vectors=all_vectors,
                chunks=all_chunks,
                embedder=EMBEDDER_NAME,
                dim=embedding_dim(),
                source_mtimes=source_mtimes,
                built_at=time.time(),
            )
            save_index(REPO_ROOT, idx)
            log(f"  checkpointed: {i}/{len(changed_paths)} files, "
                f"{len(all_chunks)} chunks total "
                f"({time.monotonic() - t0:.1f}s elapsed)")
            files_done_since_flush = 0

    log(f"wrote {len(all_chunks)} chunks ({all_vectors.shape}) to {INDEX_DIR}/")
    log(f"total embedding time: {time.monotonic() - t0:.1f}s")
    return 0


def _log(*args, **kwargs) -> None:
    print(*args, **kwargs, file=sys.stderr, flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
