"""Tests for scripts/similar/retrieve.py — index round-trip and top-K
ordering against a deterministic synthetic embedder.

Why a synthetic embedder: the unit suite must run without downloading
the 33MB BGE model. A fastembed integration test belongs in the
verification step (`just similar-build` against the real corpus).

The fake embedder maps text → 64-dim vector by counting occurrences
of a fixed vocabulary, then L2-normalizes. That preserves enough
"semantic" structure that chunks sharing keywords cluster, which is
all the retrieval pipeline needs to verify.

Uses stdlib unittest. Run via `just test-similar` or
`python3 tests/similar/test_retrieve.py -v`.
"""

from __future__ import annotations

import json
import sys
import tempfile
import time
import unittest
from pathlib import Path

import numpy as np

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "similar"))

from chunkers import chunk_ticket_or_landed                              # noqa: E402
from retrieve import (                                                   # noqa: E402
    Index, INDEX_NPZ, INDEX_META, load_index, save_index, stale_files,
    top_k, chunks_by_ticket_id,
)


# ── deterministic fake embedder ─────────────────────────────────────────────

VOCAB = [
    "schedule", "edge", "perturbation", "modifier", "pipeline", "rng",
    "feeding", "resolver", "kitten", "stockpile", "food", "meal",
    "saturation", "patrol", "shadowfox", "predator", "exposure", "ambush",
    "starvation", "labour", "bandwidth", "consideration", "score", "field",
    "balance", "hypothesis", "concordance", "iteration", "soak",
]


def fake_embed_batch(texts):
    """Score each text against `VOCAB` by lowercased substring hits,
    pad to 64 dims with zeros, L2-normalize. Deterministic and fast."""
    out = np.zeros((len(texts), 64), dtype=np.float32)
    for i, t in enumerate(texts):
        lo = t.lower()
        for j, w in enumerate(VOCAB):
            out[i, j] = float(lo.count(w))
        # Add a tiny per-text fingerprint in the tail dims so identical
        # vocab counts don't collide to the same vector.
        h = hash(t) & 0xFFFF
        out[i, 60] = (h & 0xFF) / 255.0
        out[i, 61] = ((h >> 8) & 0xFF) / 255.0
    norms = np.linalg.norm(out, axis=1, keepdims=True)
    norms[norms == 0] = 1.0
    return out / norms


# ── helpers ─────────────────────────────────────────────────────────────────

FIXTURES = Path(__file__).parent / "fixtures"


def build_synthetic_index(tmpdir: Path) -> Index:
    chunks = []
    for ticket_path in sorted((FIXTURES / "tickets").glob("*.md")):
        chunks.extend(chunk_ticket_or_landed(ticket_path, FIXTURES, "tickets"))
    vectors = fake_embed_batch([c.text for c in chunks])
    idx = Index(
        vectors=vectors,
        chunks=[c.to_dict() for c in chunks],
        embedder="fake:vocab-64",
        dim=64,
        source_mtimes={
            c.source_path: (FIXTURES / c.source_path).stat().st_mtime
            for c in chunks
        },
        built_at=time.time(),
    )
    save_index(tmpdir, idx)
    return idx


# ── tests ───────────────────────────────────────────────────────────────────

class TestIndexRoundtrip(unittest.TestCase):
    def test_save_load_roundtrip(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmpdir = Path(tmp)
            idx = build_synthetic_index(tmpdir)
            self.assertTrue((tmpdir / INDEX_NPZ).exists())
            self.assertTrue((tmpdir / INDEX_META).exists())
            loaded = load_index(tmpdir)
            self.assertEqual(loaded.embedder, "fake:vocab-64")
            self.assertEqual(loaded.dim, 64)
            self.assertEqual(loaded.vectors.shape, idx.vectors.shape)
            np.testing.assert_array_almost_equal(loaded.vectors, idx.vectors)
            self.assertEqual(len(loaded.chunks), len(idx.chunks))


class TestTopK(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.tmpdir = Path(self.tmp.name)
        self.idx = build_synthetic_index(self.tmpdir)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_query_returns_self_high_score(self) -> None:
        # When the query vector IS one of the chunk vectors, that chunk
        # should top the unfiltered results with score very close to 1.0.
        target_idx = 0
        q = self.idx.vectors[target_idx]
        hits = top_k(self.idx, q, 3)
        self.assertEqual(hits[0][0], target_idx)
        self.assertAlmostEqual(hits[0][1], 1.0, places=4)

    def test_corpus_filter_excludes_other_kinds(self) -> None:
        q = self.idx.vectors[0]
        hits = top_k(self.idx, q, 5, corpus_filter={"landed"})
        # No chunks in fixtures/tickets/ have source_kind "landed",
        # so filter to landed-only should return nothing.
        self.assertEqual(hits, [])

    def test_exclude_chunk_ids_drops_self(self) -> None:
        target_idx = 0
        q = self.idx.vectors[target_idx]
        target_chunk_id = self.idx.chunks[target_idx]["chunk_id"]
        hits = top_k(self.idx, q, 3, exclude_chunk_ids={target_chunk_id})
        self.assertNotIn(target_chunk_id, [self.idx.chunks[i]["chunk_id"] for i, _ in hits])

    def test_top_k_clamped_to_index_size(self) -> None:
        q = self.idx.vectors[0]
        hits = top_k(self.idx, q, 10000)
        self.assertEqual(len(hits), len(self.idx.chunks))

    def test_predator_query_finds_194(self) -> None:
        # Embedding the words "predator exposure ambush patrol" via the
        # fake embedder should rank ticket 194's chunks (which contain
        # those keywords) highest. Validates that the chunker preserves
        # the load-bearing prose for retrieval, not just the headers.
        q = fake_embed_batch(["predator exposure ambush patrol shadowfox"])[0]
        hits = top_k(self.idx, q, 3)
        top_ids = [self.idx.chunks[i]["chunk_id"] for i, _ in hits]
        self.assertTrue(
            any("194-saturation-suppression" in cid for cid in top_ids),
            f"expected 194 in top-3, got {top_ids}",
        )


class TestStaleFiles(unittest.TestCase):
    def test_no_changes_means_no_stale(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmpdir = Path(tmp)
            idx = build_synthetic_index(tmpdir)
            # Discover the same paths the index recorded.
            paths = [FIXTURES / p for p in idx.source_mtimes.keys()]
            self.assertEqual(stale_files(FIXTURES, idx, paths), [])

    def test_modified_file_appears_stale(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmpdir = Path(tmp)
            idx = build_synthetic_index(tmpdir)
            # Touch one fixture so its mtime changes.
            target = FIXTURES / "tickets" / "189-schedule-edge-perturbation.md"
            original_mtime = target.stat().st_mtime
            try:
                new_mtime = original_mtime + 100.0
                import os
                os.utime(target, (new_mtime, new_mtime))
                paths = [FIXTURES / p for p in idx.source_mtimes.keys()]
                stale = stale_files(FIXTURES, idx, paths)
                self.assertIn(
                    "tickets/189-schedule-edge-perturbation.md", stale,
                )
            finally:
                os.utime(target, (original_mtime, original_mtime))


class TestChunksByTicketId(unittest.TestCase):
    def test_lookup_int_or_str(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmpdir = Path(tmp)
            idx = build_synthetic_index(tmpdir)
            rows_int = chunks_by_ticket_id(idx, 189)
            rows_str = chunks_by_ticket_id(idx, "189")
            self.assertEqual(rows_int, rows_str)
            self.assertGreater(len(rows_int), 0)
            for r in rows_int:
                self.assertEqual(idx.chunks[r]["metadata"]["ticket_id"], 189)


if __name__ == "__main__":
    unittest.main(verbosity=2)
