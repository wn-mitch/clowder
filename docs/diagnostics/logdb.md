# logdb — cross-run log database (DuckDB)

`scripts/logdb.py` collates `logs/<archive>/**/events.jsonl` (and, when
explicitly requested, per-focal `trace-<cat>.jsonl` sidecars) into one
local DuckDB file at `logs/runs.duckdb`. Balance threads can then
replace hand-rolled `jq` incantations with single-line SQL across runs,
commits, and seeds.

**Complementary to `just q` (logq):** logq drills *into one run*
(fuzzy cat-name resolution, narrative joins, single-tick replay). logdb
compares *across many runs*. They share no state; pick the right tool.

## Quickstart

```bash
just logdb-build                              # ingest every logs/<dir>
just logdb-build baseline-2026-04-25          # ingest one archive (faster)
just logdb-query "SELECT COUNT(*) FROM runs"
just logdb-shell                              # interactive duckdb
just logdb-chart colony-score-over-time --archive baseline-2026-04-25
```

`logdb-build` is **idempotent** — files whose `(path, mtime)` already
match `ingested_files` are skipped. Re-run after a fresh soak; the
orchestrator runs cheap because most runs are already cached.

## Default vs opt-in tables

The daily `just logdb-build` is fast (~2 min cold on
`baseline-2026-04-25`) because the heavy tables are opt-in:

| Table | Default? | Flag | Cost (27-run baseline) |
|---|---|---|---|
| `runs`, `run_footers`, `activations` | yes | — | trivial |
| `colony_scores` (per-tick welfare) | yes | — | ~32k rows |
| `cat_snapshots` (per-snapshot) | yes | — | ~210k rows |
| `deaths` | yes | — | hundreds of rows |
| `cat_snapshot_scores` (DSE landscape) | **no** | `--with-scores` | ~2.5M rows; the dominant insert cost |
| `trace_l2`, `trace_l3`, `trace_l3_ranked` | **no** | `--with-traces` | ~1.5M rows; trace files are 700MB+ each |

The chart layer reads from `colony_scores` + `run_footers`, so it
works against the default ingest. Reach for `--with-scores` when you
need colony-wide DSE landscape queries; reach for `--with-traces` when
you need focal-cat L2/L3 drill-in across runs.

## Schema reference

Schema version is tracked in the `schema_version` table. Bumping the
constant in `scripts/logdb.py` makes `logdb-build` refuse to ingest
into a stale DB without `--rebuild`.

### `runs`

One row per run. Primary key is the deterministic surrogate
`run_id = sha256(events_path || commit_hash || mtime_ns)[:16]` so saved
queries survive a `rm logs/runs.duckdb` rebuild.

| Column | Type | Notes |
|---|---|---|
| `run_id` | VARCHAR(16) | PK |
| `archive` | VARCHAR | top-level dir under `logs/` |
| `kind` | VARCHAR | `sweep` · `trace` · `conditional` · `probe` · `canary` · `flat` |
| `seed` | INTEGER | from header `seed` |
| `rep`, `focal`, `forced_weather` | various | mutually exclusive — sweep has rep, trace has focal, conditional has weather |
| `commit_hash{,_short}`, `commit_dirty`, `commit_time` | from header | runs only comparable on identical `commit_hash` |
| `duration_secs`, `tick_start`, `tick_end` | numeric | runtime envelope |
| `events_path`, `narrative_path`, `trace_path` | VARCHAR | absolute paths to source files |
| `footer_written` | BOOLEAN | `false` ⇒ run crashed before footer write |
| `constants`, `sensory_env_multipliers` | JSON | preserved for `json_extract` queries |

Indexed on `commit_hash_short`, `commit_time`, `archive`.

### `run_footers`

PK is `run_id`. Scalar columns mirror always-present footer fields
(`positive_features_active`, `negative_events_total`,
`ward_count_final`, etc.). Open-ended dicts use **MAP columns** so the
keysets — which vary per run — stay first-class queryable:

- `deaths_by_cause MAP(VARCHAR, INTEGER)`
- `continuity_tallies MAP(VARCHAR, INTEGER)`
- `plan_failures_by_reason MAP(VARCHAR, INTEGER)`
- `interrupts_by_reason MAP(VARCHAR, INTEGER)`

Read with `tally['Starvation']`. The
`never_fired_expected_positives` array is a `VARCHAR[]`.

`final_*` mirror columns (`final_aggregate`, `final_welfare`,
`final_living_cats`, …) snapshot the last `ColonyScore` event of the
run so the across-commits chart and "ranked-by-final-score" queries
don't need a window function over `colony_scores`.

### `activations`

Long-format per-run × Feature: `(run_id, feature_name, polarity ∈
{positive,neutral,negative}, final_count)`. Sourced from the footer's
activation-tallies snapshot.

### `colony_scores`

Per-tick `ColonyScore` records (~every 100 ticks; ~1300 rows per run).
Welfare axes (`shelter`, `nourishment`, `health`, `happiness`,
`fulfillment`, `welfare`), the composite `aggregate`, the cumulative
ledger (`bonds_formed`, `kittens_born`, `deaths_*`,
`peak_population`), and `living_cats`. Indexed on `(run_id, tick)`.
This is the table the `colony-score-over-time` chart reads from.

### `cat_snapshots`

Per-snapshot record `(run_id, tick, cat)`. Flat primitives plus three
STRUCT columns for `needs` (10 axes), `personality` (18 axes), `skills`
(6 axes). Read STRUCTs as `needs.acceptance`. The `relationships`
field stays as JSON — variable-shape, low query volume.

### `cat_snapshot_scores` *(opt-in: `--with-scores`)*

Long-format DSE landscape: `(run_id, tick, cat, action, score)`. The
**only colony-wide DSE signal** — `trace_l2` exists only for the focal
cats per archive. Off by default because it's ~2.5M rows on the 27-run
baseline; opt in with `just logdb-build --with-scores` when needed.

### `deaths`

Denormalized `Death` events: `(run_id, tick, cat, cause,
injury_source, killer_species, position_x, position_y)`.

### `trace_l2`, `trace_l3`, `trace_l3_ranked` *(opt-in: `--with-traces`)*

L2: per-DSE eligibility/scoring per focal-cat tick. Variable-shape
arrays (`considerations`, `modifiers`, `top_losing`) stay as JSON;
drill in with `json_extract` + `UNNEST`.

L3: per-tick softmax outcome per focal cat. The `ranked` array is
broken out into `trace_l3_ranked (run_id, tick, cat, action,
raw_score, softmax_prob, rank)` — that's the table powering "which DSE
lost softmax mass after commit X".

**L1 records are dropped during ingest** — ~92% of trace volume, no
current consumer. To re-enable, add an `L1` branch to
`ingest_trace_file` in `scripts/logdb.py` and bump `SCHEMA_VERSION`.

### `ingested_files`, `schema_version`

Cache + version metadata. `ingested_files` keys on `file_path` with
`mtime_ns` for idempotency. Schema-version mismatch refuses ingest
without `--rebuild`.

## Example queries

### Mating cadence across baseline iterations

```sql
SELECT
    archive,
    commit_hash_short,
    commit_time,
    AVG(continuity_tallies['courtship']) AS avg_court,
    AVG(coalesce(a.final_count, 0))      AS avg_mating
FROM runs r
JOIN run_footers f USING (run_id)
LEFT JOIN activations a
       ON a.run_id = r.run_id AND a.feature_name = 'MatingOccurred'
WHERE kind = 'sweep'
GROUP BY archive, commit_hash_short, commit_time
ORDER BY commit_time;
```

### Seeds with elevated ShadowFox-attributed mortality

```sql
SELECT archive, seed, rep, deaths_by_cause['ShadowFoxAmbush'] AS fox_deaths
FROM runs JOIN run_footers USING (run_id)
WHERE deaths_by_cause['ShadowFoxAmbush'] > 5
ORDER BY fox_deaths DESC;
```

### Cats whose acceptance need crashed below 0.05 by mid-run

```sql
SELECT r.archive, r.seed, r.rep, cs.cat,
       MIN(cs.needs.acceptance) AS min_accept
FROM cat_snapshots cs
JOIN runs r USING (run_id)
WHERE cs.tick > (r.tick_start + (r.tick_end - r.tick_start) * 0.5)
GROUP BY r.archive, r.seed, r.rep, cs.cat
HAVING min_accept < 0.05
ORDER BY min_accept;
```

### DSE softmax-mass diff between two commits, for a focal *(needs `--with-traces`)*

```sql
SELECT t.action,
       AVG(t.softmax_prob) FILTER (WHERE r.commit_hash_short = 'cba19bd') AS prob_before,
       AVG(t.softmax_prob) FILTER (WHERE r.commit_hash_short = '<other>') AS prob_after
FROM trace_l3_ranked t
JOIN runs r USING (run_id)
WHERE t.cat = 'Mallow'
GROUP BY t.action
ORDER BY ABS(coalesce(prob_after,0) - coalesce(prob_before,0)) DESC;
```

### Final colony-score by archive

```sql
SELECT archive, COUNT(*) AS runs,
       AVG(final_aggregate)   AS avg_agg,
       AVG(final_welfare)     AS avg_welfare,
       AVG(final_living_cats) AS avg_alive
FROM runs JOIN run_footers USING (run_id)
WHERE kind = 'sweep'
GROUP BY archive
ORDER BY avg_agg DESC;
```

## Charts

`just logdb-chart <recipe> [args]` writes a self-contained interactive
HTML to `logs/charts/<recipe>-<ISO-timestamp>.html`. Recipes live under
`scripts/logdb_charts/`; drop a new module in to add a recipe.

### `colony-score-over-time`

Side-by-side panels:

- **Left:** `aggregate` vs `tick`, one line per run, color = run label.
- **Right:** `final_aggregate` vs `commit_time`, one point per run +
  per-archive mean line.

```bash
just logdb-chart colony-score-over-time
just logdb-chart colony-score-over-time --archive baseline-2026-04-25
just logdb-chart colony-score-over-time --seed 42 --smooth 5
just logdb-chart colony-score-over-time --commit cba19bd --max-runs 20
```

Both panels are zoom/pan-able (Altair default). Hover for tooltips.

## Adding a chart recipe

```python
# scripts/logdb_charts/my_recipe.py
import altair as alt

def register(parser):
    parser.add_argument("--my-flag", default=None)

def build(con, args):
    df = con.execute("SELECT ... FROM ...").fetchdf()
    return alt.Chart(df).mark_line().encode(x="...", y="...")
```

`logdb chart --help` lists discovered recipes via `pkgutil.iter_modules`.

## Troubleshooting

- **`schema_version` mismatch** → re-run with `--rebuild` to drop
  `logs/runs.duckdb` and ingest fresh.
- **Build is slow** → first ingest is cold; second run hits the
  `ingested_files` cache and is near-instant. To force one run to
  re-ingest, `DELETE FROM ingested_files WHERE file_path LIKE ...` in
  the shell, then re-run build.
- **`just logdb-shell` errors** → install the duckdb CLI via
  `brew install duckdb` (or `uv tool install duckdb`).

## See also

- `docs/diagnostics/log-queries.md` — the jq cookbook (single-run drill-in).
- `scripts/logq/` — the runtime behind `just q` (single-run logq).
- `src/resources/event_log.rs` — Rust source-of-truth for header/footer.
- `src/resources/colony_score.rs` — `ColonyScore` struct that drives
  `colony_scores` and `final_*` columns.
