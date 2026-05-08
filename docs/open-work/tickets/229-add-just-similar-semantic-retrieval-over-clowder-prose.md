---
id: 229
title: add just similar — semantic retrieval over Clowder prose
status: in-progress
cluster: tooling
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The Clowder repo has accumulated significant prose-heavy structure that resists keyword search: ~100 active tickets + ~150 landed tickets (each with frontmatter, audit tables, and prose-heavy Why/Scope/Decision sections), 41 balance-iteration threads, 33 system design docs, and ~5k lines of `///` doc-comments across DSEs / planner / markers. Cross-layer name divergence is real and documented (e.g. `cat_presence` emits as `congregation`, `TileMap` emits as `corruption` — see auto-memory `project_l1_map_metadata_names.md`), so a marker name doesn't grep to the DSE that scores it. CLAUDE.md mandates a layer-walk audit before any bugfix; finding *which* prior tickets, balance threads, or DSE doc-comments ring the same bell as the current investigation is currently a memory-and-grep job, and the cost shows up in friction-log entries about premise inheritance and missed adjacencies.

`just similar` adds a semantic-retrieval skill alongside `just q` / `just inspect` / `just explain` / `just verdict`. Given a ticket id, file path, or free-text query, it returns the top-K semantically-adjacent chunks across tickets / landed / balance / system docs / DSE doc-comments. Highest-ROI use: layer-walk-aid (find the DSE doc-comment that references a marker even when the names diverge across layers).

## Scope

- `scripts/similar/{similar,index,chunkers,embed,retrieve}.py` — Python tooling, fastembed/BGE-small backend, numpy cosine retrieval.
- `tests/similar/{test_chunkers,test_envelope,test_retrieve}.py` + `tests/similar/fixtures/` — hermetic fixture corpus.
- `.claude/skills/similar/SKILL.md` — skill registration mirroring the `explain` template.
- `justfile` — add `similar` and `similar-build` recipes (mirror `q` / `verdict`).
- `test-similar` recipe in justfile.
- Reuses `scripts/logq/envelope.py` (do not duplicate envelope helpers).

Corpus covered: tickets, landed, balance, pre-existing, system docs, DSE doc-comments (`///` and `//!`), planner doc-comments, markers doc-comments. Storage at `logs/.embeddings/{index.npz,index.meta.json}` (already gitignored via `/logs/*`).

## Out of scope

- Embedding non-doc-comment Rust source (grep + LSP already win).
- Cross-skill docs integration — opens as a follow-on ticket blocked-by 229 once Phase 1 lands.
- ANN libraries or vector DBs (corpus too small to justify; brute-force numpy cosine is sub-100ms at 3.7k chunks).
- Voyage / OpenAI API embedders — local fastembed + BGE-small is v1; the abstraction in `embed.py` allows future swap if needed.

## Current state

Brand-new ticket; no prior landed work. Plan file at `~/.claude/plans/is-there-a-way-zippy-dragon.md`.

## Approach

Two-phase landing. **Phase 1 (this ticket):** build the tool end-to-end — chunkers for all corpus regions, fastembed backend, numpy retrieval, justfile recipes, SKILL.md, hermetic test suite. **Phase 2 (follow-on, blocked-by 229):** splice `just similar` into `Relationship to neighbouring tools` of `.claude/skills/{logq,explain,verdict,inspect,diagnose-collapse}/SKILL.md` and add a one-line mention in CLAUDE.md "Bugfix discipline" so the tool gets reached for during layer-walk audits (per CLAUDE.md auto-memory `feedback_diagnostic_tools_need_discipline_wiring`).

Embedding model: `BAAI/bge-small-en-v1.5` via `fastembed` (local ONNX, 384-dim, 33MB model + ~50MB onnxruntime). PEP 723 inline script metadata declares the deps so `uv run` provisions on first invocation.

Chunking strategy:
- Tickets / landed: section-window (one chunk per `##` header), frontmatter as per-chunk metadata.
- Balance / system docs: section-window with sentence-window fallback for sections > 400 tokens.
- Rust doc-comments: one chunk per documented item (mod / struct / fn).
- Pre-existing: whole-file (only 2 files).

Index freshness: per-chunk `source_path` + `source_mtime` stored in `index.meta.json`. Query path emits `WARN: index stale (N files changed)` to stderr if any source mtime exceeds the stored one; doesn't auto-rebuild during query (would surprise the user with a 30s wait). `just similar-build` is incremental by default; `--full` forces rebuild.

## Verification

1. `just test-similar` (hermetic fixture corpus) — passes.
2. `just similar-build --full` — completes in < 90s.
3. `just similar 189` returns landed/189 plus adjacent substrate-refactor tickets in the top-K.
4. `just similar 'cat_presence'` returns DSE doc-comments referencing the `congregation` map (validates cross-name retrieval — the highest-ROI use case).
5. `just similar 'starvation cluster after schedule edge'` returns adjacent tickets and balance threads.
6. Top-5 query latency < 200ms (after model warm-up).
7. `just check` clean.

## Log

- 2026-05-08: ticket opened against plan `~/.claude/plans/is-there-a-way-zippy-dragon.md`.
