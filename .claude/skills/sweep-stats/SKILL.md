---
name: sweep-stats
description: Statistical summary of a sweep (`just sweep-stats <dir> [--vs <other>]`) — per-metric mean / stdev / 95% CI / sample size from every `_footer` in a sweep directory, plus Welch's t / Cohen's d / effect-size band when comparing two sweeps. Use when the user has a sweep result and wants a structured numeric summary, or when comparing two sweeps quantitatively. Trigger phrases — "is this sweep different from baseline", "Welch's t between these two sweeps", "what's the variance on metric X across the sweep", "summarize this sweep", "sweep-stats this". Do NOT fire for — single-run drift checks (use `just verdict`), per-DSE focal-trace diffs (use `just frame-diff`), hypothesis-driven runs that compose sweeps automatically (use `just hypothesize`).
---

# Sweep Statistics (`just sweep-stats`)

`just sweep-stats <SWEEP_DIR> [--vs <BASELINE_DIR>]` reads `_footer` records from every `<seed>-<rep>/events.jsonl` under a sweep directory, then emits per-metric mean / stdev / 95% CI / sample size as a JSON envelope. With `--vs <baseline-sweep-dir>` it also runs Welch's two-sample t-test, computes Cohen's d, and bands the effect.

## When to fire

**Fire when:**

- User has a sweep result (`logs/sweep-<label>/`) and wants a statistical summary.
- User is comparing two sweeps quantitatively (e.g., before/after a balance tweak that wasn't run through `just hypothesize`).
- Verifying that a `just hypothesize` result holds on a different metric than the predicted one (`hypothesize` only checks one metric; `sweep-stats --vs` checks all).
- Asking "is the difference between these two sweeps real or noise?"

**Do NOT fire when:**

- User wants survival/continuity verification on a single run — that's `just verdict`.
- User wants per-DSE drift between focal traces — that's `just frame-diff`.
- User has a hypothesis to test — that's `just hypothesize` (which uses `sweep-stats` internally).
- Sweep dir contains no `_footer` lines (run still in progress, or runs failed before footer) — tool exits 2 with "no footers found"; let the run finish first.

## The envelope

```jsonc
{
  "sweep": "logs/sweep-baseline-5b",
  "n":     15,
  "runs": [
    "42-1", "42-2", "42-3", "99-1", "99-2", "99-3",
    "7-1", "7-2", "7-3", "2025-1", "2025-2", "2025-3",
    "314-1", "314-2", "314-3"
  ],
  "metrics": [
    {
      "field":  "deaths_by_cause.Starvation",
      "mean":   2.4,
      "stdev":  0.8,
      "ci95":   [1.6, 3.2],
      "min":    0.0,
      "max":    4.0,
      "n":      15,
      "vs_baseline": {
        "delta_pct":     +15.5,
        "p":             0.0342,
        "effect_size":   +0.62,
        "band":          "drift",
        "baseline_mean": 2.1
      }
    },
    /* ... ranked by |delta_pct| when --vs is set, otherwise by field name */
  ],
  "vs_baseline": "logs/sweep-baseline-pre-shift" | null
}
```

The `vs_baseline` block on each metric is present only when `--vs <baseline-sweep-dir>` is passed.

**Exit codes:** `0` success · `2` no `events.jsonl` files found, or no `_footer` lines in any.

## Bands (the load-bearing classification)

| Band            | Criterion                                                          |
|-----------------|--------------------------------------------------------------------|
| `significant`   | \|Δ%\| ≥ 30% AND p < 0.05 AND \|Cohen's d\| > 0.5                  |
| `drift`         | 10% ≤ \|Δ%\| < 30% (worth investigating)                           |
| `noise`         | \|Δ%\| < 10%                                                       |
| `inconclusive`  | direction-only (one side missing or zero — can't compute the test) |

**Interpretation rule of thumb:** treat `significant` as "ship-blocking unless explained" and `drift` as "needs a hypothesis if it's on a characteristic metric per CLAUDE.md."

## Sweep-directory layout

`sweep-stats` walks any subdirectory of `<SWEEP_DIR>` containing an `events.jsonl`:

- `logs/sweep-<label>/<seed>-<rep>/events.jsonl` — preferred (current `just sweep` layout)
- `logs/sweep-<label>/<seed>/events.jsonl` — older single-rep layout (still recognized via `rglob` fallback)

Empty `events.jsonl` files (size 0) are skipped silently. Files without a `_footer` line are skipped (run still in progress).

## Charts (optional side effect)

`--charts` writes matplotlib boxplots per metric to `logs/charts/<sweep-name>/`. Off by default (matplotlib import is non-trivial). Use when you want a visual cross-check of the numeric envelope.

## Examples

```bash
# Single sweep — distribution stats only.
just sweep-stats logs/sweep-baseline-5b

# Two-sweep comparison — adds vs_baseline blocks per metric.
just sweep-stats logs/sweep-fog-activation-1 --vs logs/sweep-baseline-5b

# Human-readable summary.
just sweep-stats logs/sweep-X --vs logs/sweep-Y --text

# With boxplot PNGs.
just sweep-stats logs/sweep-X --charts
```

## Caveats

- **Footer fields only.** Anything not in the `_footer` line (e.g., per-tick metrics, narrative beats, trace-derived stats) won't appear. The footer is the colony-final-state summary; for tick-level dynamics use `just q events` or `just q anomalies`.
- **Welch's t assumes independence across runs in the sweep.** If your sweep ran with seeds that share state (which `just sweep` does NOT — every run is independent), the p-values are inflated.
- **n=1 means no test.** A sweep with one rep per seed and one seed gives n=1; the comparison block degenerates to direction-only `inconclusive`. Use `REPS=3` minimum for the bands to be meaningful.
- **Float metrics only.** Boolean and string fields are skipped during flattening (see `_flatten` in `scripts/sweep_stats.py:102`).
- **Underscore-prefixed fields are excluded** (`_footer`, `_header`, etc.) so they don't appear as metrics.

## Relationship to neighbouring tools

- **`just verdict`** — single-run drift vs baseline. Same band semantics, but n=1 (no t-test, no Cohen's d). Use `verdict` when you have one run; `sweep-stats` when you have a sweep.
- **`just hypothesize`** — runs two sweeps and computes concordance on **one** predicted metric. `sweep-stats --vs` is the broader cross-metric companion to `hypothesize`'s focused single-metric check.
- **`just frame-diff`** — per-DSE score-distribution diff between two focal traces. Different axis: `frame-diff` is per-DSE within-trace; `sweep-stats` is per-footer-metric across-runs.
- **`scripts/sweep_stats.py`** — implementation. Source of truth for band thresholds (`NOISE_PCT`, `SIGNIFICANT_PCT`, `P_THRESHOLD`, `EFFECT_THRESHOLD`).

## Non-goals

- Does not write to a history file. (Phase 2 of the agent-design tooling adds rationale capture, but `sweep-stats` is not yet wired into the call-history corpus.)
- Does not validate that the two sweeps have matching constants headers. **The user is responsible for confirming comparability** — pass `--text` and check the run-list lines up with what you expect.
- Does not aggregate beyond one sweep level. Nested sweeps (e.g., a sweep-of-sweeps) require manual orchestration; `sweep-stats` won't recurse.
