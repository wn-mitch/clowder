---
name: next
description: Embedding-based ready-ticket recommender (`just next`) — top-K unblocked tickets adjacent to recent landings, in-flight work, the AI substrate refactor, or an ad-hoc seed. Use when the question is "what should I work on next" and the user wants the recommendation rooted in the existing ticket/landed/system corpus, not a fresh categorical scan. Trigger phrases — "what should I work on next", "best next ticket", "pick a candidate", "what's adjacent to what I'm doing", "what's the natural follow-on to X", "rank ready tickets by relevance to <Y>". Do NOT fire for — listing all ready tickets (use `just open-work-ready`), surfacing a specific ticket's neighbors (use `just similar <id>`), pair/linkage discovery (use `just similar-linkages`), or scoring tickets by impact/effort (no signal exists for that).
---

# `just next` — embedding-based ticket recommender

`just next` returns a top-K of **ready, unblocked tickets** ranked against a configurable query vector composed from the `logs/.embeddings/` index. The index auto-refreshes when stale (incremental rebuild via `scripts/similar/index.py`, typically <3s); pass `--no-auto-rebuild` for strict read-only behavior with a stderr stale-warning instead. Designed to answer "what's the natural next thing to pick up given everything I've been doing" without re-doing a categorical browse of the index.

```bash
just next                              # blend (momentum + wip + substrate)
just next --mode momentum              # last-N landed centroid only
just next --mode wip                   # in-progress cohesion only
just next --mode substrate             # AI-refactor alignment only
just next --mode seed --seed 256       # ticket-id seed
just next --mode seed --seed "scent"   # free-text seed (use direct python for multi-word)
just next --top 10                     # widen to top-10
just next --include-blocked            # also surface status: blocked tickets
just next --include-epics              # also surface `epic: true` trackers
just next --text                       # render text envelope instead of JSON
```

## Modes

| Mode | Source set | Default weight (in `blend`) |
|------|-----------|-----------------------------|
| `momentum` | last-5 ticket centroids from `docs/open-work/landed/` (newest first) | 0.50 |
| `wip` | centroids of all `status: in-progress` tickets | 0.30 |
| `substrate` | centroid of `docs/systems/ai-substrate-refactor.md` + open Cluster A–E ticket centroids | 0.20 |
| `seed <id\|text>` | one ticket centroid OR free-text embedding | exclusive (no blend) |

Blend takes a weighted sum of available components, then L2-normalizes. Empty components (e.g. zero in-progress) drop out and remaining weights renormalize. Source-set members are **excluded from the candidate set**, so `--mode wip` won't recommend tickets currently in-progress.

Rationale: each result is paired with the highest-cosine source chunk from the query-vector source set — answers "this ticket is recommended *because it's adjacent to X*."

## When to fire

**Fire when:**

- Session start: user wants a starting point biased toward recent landings ("what should I work on next").
- Mid-session: user wonders what to round out alongside what's already in-flight ("any ready tickets that complement what I'm doing").
- After landing a ticket: surface the natural follow-on without scrolling `docs/open-work.md` ("now that 256 landed, what's adjacent").
- Substrate-refactor focus: bias the picker toward the AI epic (`--mode substrate`).
- Ad-hoc topic seeding: "rank ready tickets by relevance to flee/anxiety" → `--mode seed --seed "flee anxiety"`.

**Do NOT fire for:**

- Listing all ready tickets — that's `just open-work-ready` (no scoring).
- Finding all adjacent docs to ONE ticket — that's `just similar <id>` (returns chunks across all corpora, not just ready tickets).
- Pair/linkage discovery between two open tickets — that's `just similar-linkages`.
- Cross-reference suggestions when *creating* a new ticket — that's already wired into `just open-ticket`.
- Impact / effort / priority ranking — no signal in the index for that; the recommender is purely semantic adjacency.
- Dependency reasoning ("which ticket unblocks the most others") — out of scope; use the dependency graph in `docs/open-work.md`.

## The envelope

```jsonc
{
  "query": {
    "mode": "blend",                  // blend | momentum | wip | substrate | seed
    "seed": null,                     // ticket-id or free-text when mode=seed
    "top_k": 5,
    "landed_window": 5,               // momentum window
    "include_blocked": false,
    "weights": {                      // null when mode != blend
      "momentum": 0.5, "wip": 0.3, "substrate": 0.2
    },
    "embedder": "fastembed:bge-small-en-v1.5"
  },
  "scan_stats": {
    "indexed_chunks": 3152,
    "indexed_tickets": 265,
    "candidates": 70,                 // tickets passing readiness filter
    "query_sources": 28,              // distinct tickets feeding the query vector
    "returned": 5,
    "narrow_by": ["mode", "top", "include-blocked", "landed-window"],
    "index_stale_files": 11,
    "mode_breakdown": {"momentum": 5, "wip": 6, "substrate": 17}
  },
  "results": [
    {
      "id": "230",
      "score": 0.9464,
      "path": "docs/open-work/tickets/230-…md",
      "title": "Carve DispositionKind::Fleeing + substrate-aware flee picker",
      "cluster": "pathfinder-risk-awareness",
      "status": "ready",
      "rationale": {
        "score": 0.9279,
        "path": "docs/open-work/landed/252-…md",
        "section": "Why",
        "ticket_id": 252,
        "source_kind": "landed"
      },
      "summary": "[230] Carve DispositionKind::Fleeing … (cluster: pathfinder-risk-awareness)\n        0.95 · adjacent to landed/252 § Why"
    }
    // ...
  ],
  "narrative": "mode `blend` over 5 momentum, 6 wip, 17 substrate → 70 candidates → top 5.",
  "next": [
    "just next --mode momentum",
    "just next --mode wip",
    "just next --mode substrate",
    "just next --mode seed --seed <ticket-id>"
  ]
}
```

**Exit codes:** `0` on results · `1` on empty (no candidates passing the filter) · `2` on hard error (index missing — run `just similar-build`; or `--mode seed` without `--seed`).

## Examples

```bash
# Default blended pick — what to do next, given everything you've been doing.
just next

# Strict momentum — what's adjacent to recent landings.
just next --mode momentum

# Wider momentum window when recent landings span varied themes.
just next --mode momentum --landed-window 10

# In-flight cohesion — what rounds out what's currently open.
just next --mode wip

# Bias toward the AI substrate refactor.
just next --mode substrate

# Seed by ticket id — ready candidates closest to ticket 256.
just next --mode seed --seed 256

# Seed by free text — invoke the python script directly to preserve quoting.
# (just's {{ARGS}} expansion drops shell quotes; same limitation as `just similar`.)
uv run scripts/similar/next.py --mode seed --seed "scent marking territorial"

# Widen the top-K and render a text envelope.
just next --top 10 --text

# Reweight blend — favor substrate over momentum.
just next --w-momentum 0.2 --w-substrate 0.5

# Include status: blocked candidates (default: ready only).
just next --include-blocked

# Include `epic: true` trackers (default: epics filtered as non-workable).
just next --include-epics

# Strict read-only — skip the auto-rebuild and warn on stale index.
just next --no-auto-rebuild
```

## Index lifecycle

`just next` reads the same `logs/.embeddings/` index that `just similar` and `just similar-linkages` use. By default, if the index is stale, it spawns `scripts/similar/index.py` to refresh it before ranking (incremental, mtime-keyed; child stdout is routed to stderr so the JSON envelope stays clean) and re-loads. The auto-rebuild's progress prints `index stale (N files changed) — rebuilding incrementally...` plus the child's own progress lines to stderr. If the rebuild fails (subprocess error, missing dep), the recommender falls back to the stale-WARN path and ranks against the still-valid index rather than aborting.

Pass `--no-auto-rebuild` to skip the rebuild and reproduce the older read-only behavior: stale-WARN to stderr, then proceed against the existing index. Useful for callers that want strict determinism or want to defer rebuild cost to a later `just similar-build` invocation.

## Caveats

- **Ticket vectors are section-weighted.** Each ticket's centroid is a weighted mean over its chunks (`Why` 3.0, `Scope` 2.0, `Approach` 1.5, `Current state` 1.0, `Out of scope` / `Verification` 0.5, `Log` 0.3, everything else 1.0). Pre-2026-05-11 the centroid was an unweighted mean, which collapsed top-K spreads below the tiebreak threshold — boilerplate sections smudged tickets toward each other. See `scripts/similar/retrieve.py::SECTION_WEIGHTS`.
- **Tiebreaks are intentional but small.** When two candidates score within 0.005 (cosine), the candidate matching the dominant cluster of the highest-weighted query component gets a +0.0025 nudge. This keeps the list themed when scores are flat. Larger differences are never overridden.
- **Source exclusion is symmetric.** A ticket that contributed to the query vector is excluded from the candidate set. So `--mode wip` excludes in-progress tickets from the output (you wouldn't recommend yourself), and `--mode seed --seed 256` excludes 256 itself.
- **Epics are filtered by default.** Tickets with `epic: true` in frontmatter are tracking dashboards, not workable tickets — they're dropped from the candidate set even when their semantic similarity is high. They can still feed the source sets (momentum / wip / substrate) because they're useful as query signal. Use `--include-epics` to override.
- **`landed_on` is a string sort.** Recent landings are ranked by `YYYY-MM-DD` lexicographic order — works correctly for ISO dates but fails silently if a ticket's frontmatter has a non-ISO `landed-on` value. None observed at time of writing.
- **Substrate-mode cluster filter is exact.** Only tickets with `cluster: A`, `B`, `C`, `D`, or `E` (uppercase single letters — the AI substrate refactor phases) feed the substrate vector. Named clusters like `ai-substrate` and `substrate-migration` are NOT folded in — they're surfaced indirectly via the spec centroid, not as their own component.
- **Free-text seed via `just`.** `just next --mode seed --seed "multi word"` loses the quotes due to `just`'s `{{ARGS}}` expansion (same limitation as `just similar`). Invoke `uv run scripts/similar/next.py --mode seed --seed "multi word"` directly to preserve quoting.
- **No reranking, no MMR.** Pure cosine over the centroid; if the top-5 cluster on one theme, you get five near-duplicates. Use `--mode seed` with a more specific seed, or `--top 10` and skim, when this happens.
- **Index lag.** Newly-opened tickets aren't ranked until `just similar-build` runs. The stderr WARN names how many files are stale.

## Relationship to neighbouring tools

- **`just similar`** — semantic retrieval over the full corpus, returns chunks (not tickets); use when the question is "what other docs ring this bell" rather than "what should I do next."
- **`just similar-linkages`** — pair discovery between unlinked open tickets; use to surface conceptual neighbors that *aren't* cross-referenced, not to pick a next ticket.
- **`just open-work-ready`** — flat list of all ready tickets (id + title + cluster); use when you want the full menu, not a recommendation.
- **`just open-work-tree`** — dependency tree visualization; use when the question is "what unblocks the most work," not "what's adjacent."
- **`just open-ticket`** — uses `suggest_related_tickets()` to inject cross-references when *creating* a new ticket; orthogonal to `next`, which only ranks *existing* tickets.
- **`just verdict` → `just next`** — after a soak passes and the user wants to pick the next thing.
- **`just land <id>` → `just next`** — natural follow-on flow: land a ticket, then ask the recommender what's now most adjacent.

## Non-goals

- Does not estimate impact, effort, or priority — no such signal lives in the corpus.
- Does not reason about dependencies (`blocked-by` chains) — only filters on `status: ready`. The dependency graph in `docs/open-work.md` and `just open-work-tree` cover that axis.
- Does not auto-rebuild the index — that's `just similar-build`'s job.
- Does not write to any ticket files. Pure recommender, no side effects.
- Does not invoke the LLM — pure numpy + cosine. Sub-second after the first invocation (which loads the embedder for free-text seed).
