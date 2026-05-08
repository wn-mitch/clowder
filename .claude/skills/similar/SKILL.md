---
name: similar
description: Semantic retrieval over Clowder prose (`just similar <id-or-path-or-text>`) — top-K adjacent chunks across tickets / landed / balance / system docs / DSE doc-comments. Use whenever a layer-walk audit, bugfix triage, or balance investigation needs to find prior work that "rings the same bell" but doesn't share keywords. Trigger phrases — "any tickets like this", "what's similar to ticket N", "have we seen this before", "find related balance threads", "which DSEs mention X", "this collapse looks like…". Do NOT fire for — symbol lookup (use `rg` / LSP), cross-run SQL (`just logdb-query`), per-tick drilling (`just q`), tuning-knob explanation (`just explain`), or full-cat reports (`just inspect`).
---

# `just similar` — semantic retrieval over Clowder prose

`just similar <input>` returns the top-K semantically-adjacent chunks across the prose corpus: tickets / landed / balance / system docs / pre-existing notes / DSE doc-comments / planner doc-comments / markers doc-comments. Three input shapes are auto-detected:

```bash
just similar 189                          # ticket id (bare number) — centroid query
just similar tickets/175.md               # repo-relative file path — centroid query
just similar "saturation suppression"     # free text — direct embed
```

For ticket-id and file-path queries the query vector is the *centroid* of that source's existing chunks, and the source's own chunks are excluded from results (no self-matches). For free-text queries the text is embedded directly.

## When to fire

**Fire when:**

- A bugfix layer-walk needs the prior tickets / balance threads that ring the same bell. CLAUDE.md "Bugfix discipline" mandates a structural-option menu; `just similar <ticket-id>` surfaces precedent for split / extend / rebind / retire decisions.
- Investigating cross-layer name divergence (e.g. `cat_presence` map ↔ `congregation` metadata ↔ DSEs that score it). The keyword-only grep misses these; embedding retrieval finds them.
- A collapse fingerprint reminds the user of a past run — "does this look like the 189 schedule-edge cluster?"
- Surveying which DSE doc-comments mention a concept (`just similar 'predator exposure' --corpus dses,planner`).
- Drafting a balance hypothesis and wanting prior threads on the same metric (`just similar 'starvation cluster' --corpus balance`).

**Do NOT fire for:**

- Symbol lookup — `rg` / LSP / `cargo doc` are sharper.
- Per-tick drilling into a single run — that's `just q` / focal-cat traces.
- Tuning-knob value lookup or read-site enumeration — that's `just explain`.
- Aggregate cat reports — that's `just inspect`.
- Cross-run SQL queries — that's `just logdb-query`.
- Memory-service retrieval (auto-memory entries) — that's the MCP memory tools, which hold *learned* patterns, not raw prose.

## The envelope

```jsonc
{
  "query": {
    "input": "189",
    "input_kind": "ticket_id",        // ticket_id | file_path | free_text
    "top_k": 5,
    "corpus": ["tickets", "landed", "balance", "pre-existing", "systems",
               "dses", "planner", "markers"],
    "embedder": "fastembed:bge-small-en-v1.5"
  },
  "scan_stats": {
    "scanned": 3712,
    "returned": 5,
    "more_available": true,
    "narrow_by": ["corpus", "top_k"],
    "index_stale_files": 0
  },
  "results": [
    {
      "id": "landed/189-schedule-edge-perturbation:Why",
      "score": 0.812,
      "path": "docs/open-work/landed/189-schedule-edge-perturbation.md",
      "section": "Why",
      "source_kind": "landed",
      "metadata": {
        "ticket_id": 189, "title": "schedule-edge perturbation in modifier pipeline",
        "status": "done", "cluster": "ai-substrate", "landed_on": "2026-04-12"
      },
      "summary": "schedule-edge perturbation in modifier pipeline — § Why"
    }
    // ...
  ],
  "narrative": "5 chunks across 4 documents (3 landed, 1 balance, 1 dses).",
  "next": [
    "just similar 189 --corpus landed",
    "just similar 189 --corpus dses,planner,markers",
    "just similar 189 --top-k 10"
  ]
}
```

**Exit codes:** `0` on any results · `1` on empty results (narrative explains; try `--top-k` higher or drop `--corpus`) · `2` on hard error (no index built — run `just similar-build`; or unknown corpus filter).

## Examples

```bash
# Ticket id — centroid of all chunks for ticket 189, excludes self.
just similar 189

# Narrow to landed-only — useful for "have we shipped a fix for this?".
just similar 189 --corpus landed

# Search across DSE / planner / markers doc-comments — layer-walk aid.
just similar 'predator exposure' --corpus dses,planner,markers

# File path — centroid of all chunks belonging to that file.
just similar tickets/175.md

# Free-text query.
just similar 'starvation cluster after schedule edge'

# Larger result set.
just similar 189 --top-k 20

# Human-readable text envelope.
just similar 189 --text

# Force a full index rebuild before querying (rare — usually
# `just similar-build` directly is cheaper).
just similar 189 --rebuild
```

## Index lifecycle

- **First use:** `just similar-build --full` builds the full index (~60–90 seconds). Stored in `logs/.embeddings/` (gitignored via `/logs/*`). Downloads the `BAAI/bge-small-en-v1.5` model on first invocation (~33MB → `~/.cache/fastembed/`).
- **Daily use:** `just similar-build` runs incrementally, re-embedding only files whose mtime exceeds the recorded one. Vanished files have their chunks dropped automatically.
- **Stale warnings:** if you query without rebuilding after editing a corpus file, `just similar` prints `WARN: index stale (N files changed) — run 'just similar-build' to refresh` to stderr and proceeds with the stale index. Doesn't auto-rebuild during a query — would surprise you with a 30s wait.
- **Embedder change:** if `embed.py::EMBEDDER_NAME` changes, `just similar-build` (without `--full`) detects the mismatch and forces a full rebuild. Existing chunks aren't compatible with a new embedding space.

## Caveats

- **First-invocation cost.** First `just similar-build` downloads the BGE model (~33MB). Cached in `~/.cache/fastembed/` thereafter; subsequent invocations are instant.
- **BGE quality on Rust doc-comments.** The embedder was trained on natural-language web text. Doc-comments mixing prose with type signatures (`Has<>`, `Without<>`, `MarkerSnapshot`) may retrieve weaker than pure prose. If a query like `just similar 'cat_presence'` doesn't surface the expected DSE doc-comments, fall back to keyword grep — and consider whether the symbol's prose explanation lives in `docs/systems/` or a balance doc instead of the source comment.
- **Stale section windows.** Each `## Heading` is one chunk. A ticket whose Why is in section A and whose Decision is in section B will return them as separate results; sometimes you need to read both. Use `--top-k 10` to widen the slice.
- **No reranking.** Pure cosine similarity over BGE embeddings. Future work: cross-encoder rerank for top-50 → top-5. Not implemented because the corpus is small and the headline use cases (ticket adjacency, balance-thread retrieval) are well-served by raw cosine.
- **Centroid queries blur multi-topic tickets.** A ticket spanning two unrelated concerns produces a centroid that's near neither — free-text queries on the specific concern work better. Future work: per-section query mode (`just similar 189:Approach`).

## Relationship to neighbouring tools

- **`just q`** — drills into ONE run's events.jsonl. Orthogonal axis: `q` answers "what happened in this run", `similar` answers "what's like this across all docs."
- **`just explain`** — explains a tuning constant (doc-comment + read-sites + Spearman rho). Use `explain` first when the question is "what does X do"; use `similar` when the question is "what other knobs / tickets / threads relate to X."
- **`just inspect`** — aggregate cat report from a run. Single-run / single-cat axis vs. cross-document axis.
- **`just verdict`** — pass/concern/fail gate for a soak. Use `verdict` for verification; if it fails, use `similar` against the failing-cat fingerprint or against a reframed-collapse description to find precedent.
- **`just diagnose-collapse`** — overview of a collapse. After it names a cluster (e.g. "189-style schedule-edge"), `similar 189` surfaces the prior fix.
- **logdb (`just logdb-query`)** — cross-run SQL. logdb compares numbers across runs; `similar` retrieves prose across docs. Often complementary: logdb identifies a metric drift, `similar` surfaces the balance-doc thread that explains it.
- **memory service** — auto-memory holds *learned* patterns ("integration tests must hit a real database"). `similar` retrieves *raw* prose from the canonical artifacts (tickets, balance docs, source doc-comments). Memory tells you the rule; `similar` tells you the precedent the rule came from.

## Non-goals

- Does not embed Rust code bodies (only doc-comments). For symbol-level navigation, use `rg` / LSP / `cargo doc`.
- Does not embed the contents of `logs/` (the run artifacts). For run analysis, use `just q` and friends.
- Does not auto-rebuild during a query. Index freshness is the user's call — `just similar-build` is fast enough that running it after landings is reasonable; running it on every query would burn ~30s for no value most of the time.
- Does not replace the memory service. Memory holds learned rules; this tool retrieves raw prose. Both have their place during a layer-walk.
