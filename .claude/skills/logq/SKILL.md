---
name: logq
description: Drill into a Clowder sim run with parameterized log queries (`just q …`). Use whenever the user is investigating a specific run, cat, tick range, or anomaly — after `/diagnose-run` surfaces something interesting, or before it, when they already know what they're looking for. Produces a consistent envelope (query echo, scan stats, stable IDs, narrative, suggested next queries). Trigger phrases — "why did <cat>", "what happened at tick N", "show me the <event> events", "drill into X", "look at run <path>", "was there a starvation spike", "which DSE did <cat> pick most". Do NOT fire for — the overview report (that's `/diagnose-run`'s job), editing sim source code, rebalancing constants without looking at a run.
---

# Log Query Surface (`just q`)

Clowder stores sim runs as JSONL bundles (`logs/tuned-<seed>/{events,narrative,trace-*}.jsonl`). The `just q` surface is nine query subtools over those files, each returning a standard envelope. Use them when the user wants to investigate a run — not when they want the fixed overview report (that's `/diagnose-run`).

## When to fire

**Fire when:**

- User has a specific question about a run ("did anything happen at tick 12000?", "why did Simba starve?", "which cats died this run?", "show me the Legend-tier narrative").
- User just got a `/diagnose-run` report and wants to drill into one of its sections or one of its "Threads to pull" suggestions.
- User references a log directory and wants to do anything other than a full diagnose overview.
- User asks to compare two runs — use `run-summary` on each, diff the constants via the existing `log-queries.md` §2 diff-constants recipe.

**Do NOT fire when:**

- User asks for the overview report — that's `/diagnose-run`, which is deliberately diff-stable and fixed-shape. Do not call `just q` to reconstruct it.
- User is editing simulation source (`src/**`) or tuning constants without referencing a specific logged run.
- User invokes `rank-sim-idea` (that rubric doesn't query runs).

## The envelope (every `just q` subtool returns this shape)

```jsonc
{
  "query":       { /* effective query echo, including applied defaults */ },
  "scan_stats":  { "scanned": N, "returned": K, "more_available": bool,
                   "narrow_by": ["kind", "tick_range", ...] },
  "results":     [ /* each has a stable `id` (e.g. "tick:3812:Death:Simba") */ ],
  "narrative":   "One-sentence gloss of what was found.",
  "next":        [ /* suggested follow-up `just q` commands */ ]
}
```

Rules the tool consumers (you) should rely on:

- **Null results carry nearest-match evidence.** When `results == []`, read `narrative` — it names the nearest ticks/records the tool found. Don't re-query with wider bounds reflexively; the narrative usually tells you exactly where to look.
- **Stable IDs are handles.** `tick:3812:Death:Simba` → next command is `just q cat-timeline logs/tuned-42 Simba --tick-range=3700..3900`. Don't pass IDs verbatim as params; decompose them.
- **`scan_stats.more_available = true`** means pagination; widen `--limit` or narrow with the fields in `narrow_by`.
- **Follow `next` hints when they fit.** They're optional — prefer them over inventing a new query, but override when the user's question points elsewhere.

## Subtools

| Command                                                    | Use when                                                                               |
|------------------------------------------------------------|----------------------------------------------------------------------------------------|
| `just q run-summary <log_dir>`                             | Orient on a run. Header, footer (curated fields incl. derived `final_tick` + top-3 interrupt/plan-failure reasons), joinability of trace sidecars. |
| `just q footer <log_dir> [--field=NAME] [--top-keys=N]`    | Drill into any footer field. Without `--field` lists every top-level key; with it ranks dict entries (e.g. `--field=interrupts_by_reason --top-keys=5`). |
| `just q actions <log_dir> [--cat=NAME] [--tick-range]`     | DSE-balance check. Aggregates `current_action` from `CatSnapshot` events. The first stop after a soak when you want to know "what are cats actually doing?". |
| `just q events <log_dir> [--kind=X,Y] [--tick-range] [--cat] [--limit] [--offset]` | Generic filter. Start broad and narrow.                                                |
| `just q deaths <log_dir> [--cause] [--tick-range] [--cat]` | Deaths — high-signal event type.                                                       |
| `just q narrative <log_dir> [--tier=Legend,Danger,Significant] [--tick-range]` | Story beats. Default excludes Action/Micro noise.                                      |
| `just q trace <log_dir> <cat> [--layer=L1\|L2\|L3] [--tick-range] [--top-dses=N]` | Per-cat decision layer. L3 = chosen action; L2 = DSE evaluations; L1 = influence maps. |
| `just q cat-timeline <log_dir> <cat> [--tick-range] [--limit=N] [--offset=N] [--summarize]` | One cat's events + narrative mentions. Defaults to a 50-row page; pass `--limit=0` for full stream or `--summarize` for aggregates (event-type distribution, plan-create cadence — auto-flags plan-churn under 5 ticks/plan). |
| `just q anomalies <log_dir>`                               | Curated scan: canaries, continuity tallies, ColonyScore cliffs.                        |

All subtools default to `--format json` (for you). If the user wants to read output at the CLI, add `--format text`.

## Sweep directories

When `<log_dir>` is a sweep root (`logs/sweep-<label>/<seed>-<rep>/events.jsonl`), the surface auto-detects and switches mode:

- `just q deaths <sweep_dir>` — per-cause incidence (`Starvation: 10/15 runs (66.7%)`) plus per-run breakdown. Variance is the signal: "10/15 runs starved" is a different finding from "1 run starved".
- `just q anomalies <sweep_dir>` — per-canary `[systemic|majority|seed-variant]` severity. Systemic = fired in every run (structural), seed-variant = fired in <50% (chase the diff). Emits diff-pair `next` commands for seed-variant anomalies — one fired run + one clean run, ready to diff.
- `just q events <sweep_dir>` — refuses (the union dump would be unreadable). Suggests a single-run dir or `q deaths` / `q anomalies`.

Single-run subtools (`run-summary`, `narrative`, `trace`, `cat-timeline`, `actions`, `footer`) still need a single-run dir.

## Did-you-mean and close-calls

Two adjacency behaviours fire automatically when relevant:

- **Fuzzy cat-name suggestions.** `just q trace logs/x Simbo` → `narrative: "Did you mean: Simba?"` and `next: just q trace logs/x Simba ...`. Same for `cat-timeline` and `actions --cat`. Fires only on the empty-result error path; doesn't bloat normal output.
- **Close-call ticks in `q trace --layer=L3`.** Ticks where the softmax top-1 vs top-2 probability margin < 0.10 are surfaced as a single results row (count + first 5 ticks) plus a literal `--tick-range=N..N --layer=L2` drilldown in `next`. These are the *diagnostic* ticks — flat-winner ticks tell you nothing; close calls are where momentum, jitter, or a whisker of difference flipped the action.

## Investigation recipes

**"Why did this run look off?"**

1. `just q run-summary <log_dir>` — orient, confirm commit, confirm trace-sidecar joinability.
2. `just q anomalies <log_dir>` — the curated failure sweep. Each result carries a `next` suggestion.
3. Follow the `next` hints.

**"Deep-dive on one cat."**

1. `just q cat-timeline <log_dir> <cat>` — events + narrative mentions.
2. If the run has a trace sidecar: `just q trace <log_dir> <cat> --layer=L3` for chosen-action distribution.
3. `--layer=L2` with `--tick-range` around an interesting moment for the DSE evaluation breakdown.

**"Did <feature> ever fire?"**

`just q events <log_dir> --kind=FeatureActivated --limit=20` — if returns 0, the `narrative` field will point you at the nearest tick where *any* FeatureActivated event fired.

**"Compare two runs."**

Two `just q run-summary` envelopes, then run the `log-queries.md` §2 `diff-constants` recipe directly. No `just q` subtool does run-diffing yet.

## Always pass `--rationale "<why>"` when called by an agent

Every invocation appends a record to `logs/agent-call-history.jsonl` capturing the subtool, args, exit code, and rationale. The rationale is the *intent* — what you were actually trying to figure out — not a description of the args (which are recorded separately).

Good rationales:
- `--rationale "drilling into deaths after verdict flagged starvation"`
- `--rationale "what action distribution did Simba have this run?"`
- `--rationale "did FeatureActivated fire for ShadowFoxBanished?"`

Skip the flag only when running by hand from the CLI. The flag goes on the **top-level** parser, before the subtool name:

```bash
just q --rationale "<why>" <subtool> <log_dir> [flags]
```

## Relationship to `/diagnose-run` and to `log-queries.md`

- **`/diagnose-run`** produces a fixed-shape, diff-stable *report*. `just q` produces *query results*. Don't conflate them. `/diagnose-run`'s trailing "Threads to pull" section (when present) emits `just q …` commands — follow them.
- **`docs/diagnostics/log-queries.md`** is the source of truth for the underlying jq recipes. Each subtool's code cites the section it wraps. If `just q` disagrees with the recipe, the recipe wins; file the bug.

## Troubleshooting

- `FileNotFoundError: logs/.../events.jsonl` → the run dir is wrong, or the soak hasn't written yet. Tell the user to run `just soak <seed>` or `just soak-trace <seed> <focal>`.
- Trace file missing for `just q trace` → the run was a non-trace soak. Re-run with `just soak-trace`.
- Trace `joinable=NO` in `run-summary` → events and trace came from different commits/seeds; do not cross-reference their data. Re-run the pair.
