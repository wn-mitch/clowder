"""Tests for scripts/similar/chunkers.py.

Pure-Python — no embedding model required. Asserts:
- frontmatter parsing roundtrips ticket metadata
- section-window chunker produces one chunk per `## Heading`
- Rust doc-comment chunker extracts both `//!` (module) and `///`
  (item-level) blocks, ignoring undocumented items
- chunk_path dispatches based on repo-relative prefix

Uses stdlib unittest. Run via `just test-similar` or
`python3 tests/similar/test_chunkers.py -v`.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "similar"))

from chunkers import (  # noqa: E402
    chunk_path,
    chunk_balance_or_systems,
    chunk_rust_doc_comments,
    chunk_ticket_or_landed,
    chunker_for,
    discover_corpus_files,
)

FIXTURES = Path(__file__).parent / "fixtures"


class TestTicketChunker(unittest.TestCase):
    def setUp(self) -> None:
        self.path = FIXTURES / "tickets" / "189-schedule-edge-perturbation.md"

    def test_chunks_split_per_section(self) -> None:
        chunks = list(chunk_ticket_or_landed(self.path, FIXTURES, "tickets"))
        sections = [c.section for c in chunks]
        # The fixture has Why / Approach / Verification sections.
        self.assertIn("Why", sections)
        self.assertIn("Approach", sections)
        self.assertIn("Verification", sections)

    def test_frontmatter_metadata_attached(self) -> None:
        chunks = list(chunk_ticket_or_landed(self.path, FIXTURES, "tickets"))
        why = next(c for c in chunks if c.section == "Why")
        self.assertEqual(why.metadata["ticket_id"], 189)
        self.assertEqual(why.metadata["status"], "done")
        self.assertEqual(why.metadata["cluster"], "ai-substrate")
        self.assertEqual(why.metadata["landed_on"], "2026-04-12")

    def test_chunk_id_is_stable(self) -> None:
        chunks = list(chunk_ticket_or_landed(self.path, FIXTURES, "tickets"))
        ids = [c.chunk_id for c in chunks]
        self.assertIn("tickets/189-schedule-edge-perturbation:Why", ids)
        # Asserts deterministic ordering — IDs match section order.
        ids_again = [c.chunk_id for c in chunk_ticket_or_landed(
            self.path, FIXTURES, "tickets")]
        self.assertEqual(ids, ids_again)

    def test_synthetic_header_carries_context(self) -> None:
        chunks = list(chunk_ticket_or_landed(self.path, FIXTURES, "tickets"))
        why = next(c for c in chunks if c.section == "Why")
        # The header line should mention ticket id + status so the
        # embedder picks up location, not just content.
        self.assertIn("ticket 189", why.text)
        self.assertIn("status: done", why.text)
        self.assertIn("schedule-edge", why.text)


class TestBalanceChunker(unittest.TestCase):
    def test_balance_doc_no_frontmatter(self) -> None:
        path = FIXTURES / "balance" / "sample.md"
        chunks = list(chunk_balance_or_systems(path, FIXTURES, "balance"))
        sections = {c.section for c in chunks}
        self.assertIn("Hypothesis", sections)
        self.assertIn("Observation", sections)
        self.assertIn("Concordance", sections)
        # Header line attribution.
        sample_chunk = next(iter(chunks))
        self.assertIn("balance/sample", sample_chunk.text)


class TestRustDocChunker(unittest.TestCase):
    def setUp(self) -> None:
        self.path = FIXTURES / "dses" / "sample.rs"

    def test_module_doc_extracted(self) -> None:
        chunks = list(chunk_rust_doc_comments(self.path, FIXTURES, "dses"))
        kinds = [c.metadata["item_kind"] for c in chunks]
        self.assertIn("mod", kinds)
        mod_chunk = next(c for c in chunks if c.metadata["item_kind"] == "mod")
        self.assertIn("test fixtures", mod_chunk.text)

    def test_item_doc_extracted(self) -> None:
        chunks = list(chunk_rust_doc_comments(self.path, FIXTURES, "dses"))
        item_names = [c.metadata.get("item_name") for c in chunks]
        self.assertIn("score_patrol", item_names)
        self.assertIn("score_mate", item_names)

    def test_item_decl_in_text(self) -> None:
        chunks = list(chunk_rust_doc_comments(self.path, FIXTURES, "dses"))
        patrol = next(c for c in chunks
                      if c.metadata.get("item_name") == "score_patrol")
        # The declaration should be in the embed text so retrieval can
        # find by signature, not just docstring prose.
        self.assertIn("pub fn score_patrol", patrol.text)
        # The docstring's reference to ticket 194 survives chunking.
        self.assertIn("ticket 194", patrol.text)

    def test_chunk_id_format(self) -> None:
        chunks = list(chunk_rust_doc_comments(self.path, FIXTURES, "dses"))
        ids = {c.chunk_id for c in chunks}
        self.assertIn("dses/sample:mod!", ids)
        self.assertIn("dses/sample:fn:score_patrol", ids)


class TestDispatcher(unittest.TestCase):
    def test_known_prefixes_route(self) -> None:
        self.assertEqual(
            chunker_for("docs/open-work/tickets/229-foo.md"),
            ("tickets", "ticket_or_landed"),
        )
        self.assertEqual(
            chunker_for("src/ai/dses/socialize.rs"),
            ("dses", "rust_doc_comments"),
        )
        self.assertEqual(
            chunker_for("src/components/markers.rs"),
            ("markers", "rust_doc_comments"),
        )

    def test_unknown_prefix_returns_none(self) -> None:
        self.assertIsNone(chunker_for("README.md"))
        self.assertIsNone(chunker_for("src/main.rs"))

    def test_chunk_path_dispatches_to_right_chunker(self) -> None:
        # Wire chunker_for + chunk_path against the fixture root.
        # The fixture layout mirrors docs/open-work/tickets/ vs balance/
        # via a custom prefix; chunk_path won't find them under the real
        # prefixes, so we invoke the chunkers directly above and rely
        # on chunker_for() for the dispatch logic test.
        # This test asserts the empty-file guard.
        empty = FIXTURES / "tickets" / "_empty.md"
        empty.write_text("")
        try:
            self.assertEqual(chunk_path(empty, FIXTURES), [])
        finally:
            empty.unlink()


class TestDiscoverCorpus(unittest.TestCase):
    def test_walks_real_repo_corpus(self) -> None:
        # Uses the actual repo (REPO_ROOT) — sanity check that the
        # discoverer finds files and skips templates.
        paths = discover_corpus_files(REPO_ROOT)
        rel = [str(p.relative_to(REPO_ROOT)) for p in paths]
        # No template files leaked.
        self.assertFalse(
            any("_template" in r for r in rel),
            f"template file leaked: {[r for r in rel if '_template' in r][:3]}",
        )
        self.assertFalse(
            any(r.endswith("/open-work.md") for r in rel),
            "open-work.md index leaked into corpus",
        )
        # Found at least some markdown and rust files.
        self.assertTrue(any(r.endswith(".md") for r in rel))
        self.assertTrue(any(r.endswith(".rs") for r in rel))


if __name__ == "__main__":
    unittest.main(verbosity=2)
