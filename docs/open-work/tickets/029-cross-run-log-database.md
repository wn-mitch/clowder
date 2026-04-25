---
id: 029
title: Cross-run log database — collate baseline + diagnostic archives for SQL-style queries
status: ready
cluster: null
added: 2026-04-25
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Comparing balance work across multiple runs is friction-heavy today.
Every drill-down (which DSEs lose softmax across seeds, how a
canary's distribution shifts between commits, which constants
correlate with predator-active mortality, …) requires hand-rolled
`jq` over tens of GB of JSONL spread across `logs/tuned-*`,
`logs/baseline-*`, `logs/sweep-*`, and per-phase diagnostic
archives.

The new `just baseline-dataset` orchestrator landed at
`bced533` produces 27 footer-complete runs and 8.9 GB of JSONL per
invocation. As baseline iterations stack up — one per balance-work
milestone — the comparison surface grows quadratically. Every
balance-thread session currently re-invents the same parsing
gymnastics:

- "show me MatingOccurred across seeds × commits"
- "which seeds had ShadowFox-attributed mortality > 5"
- "DSE final-score landscape diff between commit X and commit Y for
  focal Mallow"
- "all CatSnapshots where need.acceptance < 0.05"

A queryable store with header / footer / event / trace dimensions
flattens this work into single-line SQL.

The natural shape of the data (immutable per-run, write-once,
header-tagged with commit hash) maps onto a cheap embedded analytics
store — DuckDB is the obvious target.

## Scope

A new `scripts/logdb.py` plus a `just logdb-build` recipe that
ingests an arbitrary set of `logs/<archive>/` directories into a
single DuckDB file (`logs/runs.duckdb`). Schema below; subject to
revision after the first ingest exposes corner cases.

### Tables

- **`runs`** — one row per run. PK is `(label, seed, rep_or_focal,
  weather)` plus the `commit_hash`. Columns:
  - `archive` (e.g. `baseline-2026-04-25`)
  - `kind` (`sweep` | `trace` | `conditional`)
  - `seed`, `rep_or_focal`, `forced_weather`
  - `commit_hash`, `commit_hash_short`, `commit_dirty`,
    `commit_time`
  - `duration_secs`, `tick_start`, `tick_end`
  - `events_path`, `narrative_path`, `trace_path`
  - `footer_written` (bool)
  - The full `constants` and `sensory_env_multipliers` JSON blobs
    so commit-comparable invariants are queryable.

- **`run_footers`** — flattened per-run footer. PK is the run.
  Columns: every `deaths_by_cause.*` key, every
  `continuity_tallies.*` key, `never_fired_expected_positives` (as
  array → struct), and the activation-tallies snapshot.

- **`activations`** — long-format per-run × Feature, so trend
  queries don't need to deserialize SystemActivation maps.
  Columns: `run_pk, feature_name, polarity ('positive' | 'neutral'
  | 'negative'), final_count`.

- **`cat_snapshots`** — flattened CatSnapshot records. One row per
  (run, tick, cat). Columns: `run_pk, tick, cat, current_action,
  life_stage, sex, orientation, season, mood_valence, health,
  corruption, magic_affinity, social_warmth` plus a `needs`
  struct (10 axes), a `personality` struct (18 axes), a `skills`
  struct (6 axes), and a `last_scores` array for DSE-landscape
  queries.

- **`deaths`** — denormalized death events. Columns: `run_pk, tick,
  cat, cause, injury_source, killer_species` (when applicable).

- **`trace_l2`**, **`trace_l3`** — trace records from focal-cat
  sidecars. Same denormalization pattern as cat_snapshots.

### CLI

```
just logdb-build                  # ingest every logs/baseline-* and logs/sweep-*
just logdb-build LABEL            # ingest only the named archive
just logdb-query SQL              # one-shot query, prints table to stdout
just logdb-shell                  # interactive duckdb session
```

`logdb-build` is idempotent — runs whose `(events_path, mtime)`
already match the database are skipped. Re-ingest after a fresh
soak by re-invoking; the orchestrator runs cheap because most runs
are already cached.

### Example queries

```sql
-- Mating cadence across all baseline iterations
SELECT archive, commit_hash_short, AVG(continuity_tallies['courtship']) AS avg_court,
       AVG(activations.final_count) FILTER (WHERE feature_name='MatingOccurred') AS avg_mating
FROM runs JOIN run_footers USING (run_pk) JOIN activations USING (run_pk)
WHERE kind='sweep'
GROUP BY archive, commit_hash_short
ORDER BY commit_time;

-- DSE final-score landscape diff between two commits, for a focal
SELECT t1.dse, AVG(t1.final_score) AS commit_a, AVG(t2.final_score) AS commit_b,
       AVG(t2.final_score) - AVG(t1.final_score) AS delta
FROM trace_l2 t1 JOIN runs r1 USING (run_pk)
LEFT JOIN trace_l2 t2 ON t1.tick = t2.tick AND t1.dse = t2.dse
LEFT JOIN runs r2 ON t2.run_pk = r2.run_pk
WHERE r1.commit_hash_short='cba19bd' AND r2.commit_hash_short='<future>'
  AND r1.rep_or_focal='Mallow' AND r2.rep_or_focal='Mallow'
GROUP BY t1.dse
ORDER BY ABS(delta) DESC;

-- All cats whose acceptance need crashed below 0.05 by Q3
SELECT run_pk, cat, MIN(needs.acceptance) AS min_acceptance
FROM cat_snapshots
WHERE tick > (tick_start + (tick_end - tick_start) * 0.5)
GROUP BY run_pk, cat
HAVING min_acceptance < 0.05;
```

## Out of scope

- Hosting the database remotely or sharing it across machines. Each
  developer's `logs/runs.duckdb` is local; cross-machine sharing is
  a separate problem (rsync, S3, …).
- Streaming ingestion during a live soak. Today's soaks complete in
  ~15 min and ingestion is post-hoc; live ingestion is unnecessary
  complexity until a soak takes long enough that mid-run insight
  matters.
- Visualization. SQL → table is enough for now; charting layers can
  hang off the DB later (the existing `tools/narrative-editor`
  dashboard is a candidate consumer).
- Replacing `scripts/logq/`. logq is the right tool for "drill into
  one run" — fuzzy cat-names, narrative joins, single-tick replay.
  logdb is for "compare across many runs". They're complementary;
  keep both.

## Current state

Just `logs/baseline-2026-04-25/` exists today (8.9 GB, 27 runs,
27 footer-complete). Other archives in `logs/` (e.g.
`tuned-42-iter2-batch3`, `phase4c7-baseline`) would also benefit
from being indexed. Estimated database size from a single baseline
archive: **~50–100 MB** post-compression — DuckDB's columnar Parquet
backing makes this very cheap.

## Approach

1. Sketch schema against `logs/baseline-2026-04-25/`. Iterate once
   on the L2/L3 trace tables since they're the largest dimension.
2. Implement `scripts/logdb.py` ingest. Use `pyarrow` for the
   JSONL → Parquet transform, then `duckdb` for the table-create
   layer. Skip records whose checksum matches the cache.
3. Wire the four `just logdb-*` recipes.
4. Backfill ingest of every `logs/baseline-*` and `logs/sweep-*`
   currently on disk.
5. Document the schema + a few example queries in
   `docs/diagnostics/logdb.md`. Add a "look at logdb first" pointer
   to CLAUDE.md's Simulation Verification section.

## Verification

- `just logdb-build` completes without error against the existing
  baseline-2026-04-25 archive in under 5 minutes.
- `just logdb-query "SELECT COUNT(*) FROM runs"` returns 27.
- `just logdb-query "SELECT DISTINCT commit_hash_short FROM runs"`
  returns exactly one row (`cba19bd`) — header parity is preserved
  in the SQL surface.
- A single example query from the docs runs in < 5 seconds.
- A regression test: re-running `just logdb-build` with no new logs
  is a no-op (idempotency).

## Log

- 2026-04-25: Ticket opened. User flagged friction of
  hand-rolled `jq` across 27-run datasets when investigating the
  mating cadence and play canary regressions; the cross-run
  comparison surface is now load-bearing for every balance thread.
