---
name: baseline-dataset
description: Build a versioned baseline dataset (`just baseline-dataset <label>`) — five-phase orchestrator that runs a probe pass, an aggregate sweep, focal-cat traces, conditional weather variants, and a REPORT.md generation. Use when the user wants to lock in a new comparison baseline (the kind `just verdict` will read via `logs/baselines/current.json` or `just promote`). Trigger phrases — "build a versioned baseline", "rebuild the baseline dataset", "promote a new healthy baseline", "we need a fresh baseline against which to measure". Do NOT fire for — single-seed verification (use `just soak` + `just verdict`), one-off sweep work (use `just sweep`), drift checks against an existing baseline (use `just verdict` or `just sweep-stats --vs`).
---

# Versioned Baseline Orchestrator (`just baseline-dataset`)

`just baseline-dataset <LABEL>` is the five-phase pipeline that produces a complete versioned-baseline directory under `logs/baseline-<LABEL>/`. Backgroundable: writes `STATUS.txt` + `STATUS.json` after every phase so external watchers can poll progress without parsing the run log.

## When to fire

**Fire when:**

- User wants to lock in a new "healthy colony" baseline that future `just verdict` calls will compare against.
- After a substrate refactor or major balance change has settled and the old baseline is no longer representative.
- Quarterly or after a significant constants overhaul (cf. `just rebuild-sensitivity-map` cadence).

**Do NOT fire when:**

- User just wants one soak verified (`just soak` + `just verdict`).
- User wants treatment-vs-baseline drift on one constants change (`just hypothesize`).
- User wants to compare two existing sweeps (`just sweep-stats --vs`).
- Working tree is dirty without `ALLOW_DIRTY=1` set — the orchestrator will hard-fail on the dirty check (this is intentional; baselines must be reproducible).

## Output shape — directory tree, not JSON

`baseline-dataset` writes a structured directory under `logs/baseline-<LABEL>/`:

```
logs/baseline-<LABEL>/
├── STATUS.txt              ← human-readable progress (live-tailable)
├── STATUS.json             ← machine-readable progress mirror (poll without sed/awk)
├── rosters.json            ← Phase 1 probe output (per-seed cat rosters)
├── sweep/                  ← Phase 2: <SEEDS> × <REPS> × <DURATION> long soaks
│   └── <seed>-<rep>/
│       ├── events.jsonl
│       └── narrative.jsonl
├── trace/                  ← Phase 3: focal-cat traces (5 seeds × 2 focals × 900s)
│   └── <seed>-<focal>/
│       ├── events.jsonl
│       └── trace-<focal>.jsonl
├── conditional/            ← Phase 4: seed 42 × {fog, storm} × 900s
│   └── <variant>/
│       └── events.jsonl
├── canaries/               ← canary check outputs per phase
└── REPORT.md               ← Phase 5: aggregated summary
```

**Phase completion** is detected by the presence of a `_footer` line in `events.jsonl`. Re-invocation skips already-complete phases idempotently.

**STATUS.json schema:**

```jsonc
{
  "label": "2026-04-25",
  "started_at": "2026-04-25T08:14:00-04:00",
  "updated_at": "2026-04-25T09:42:11-04:00",
  "phase": "phase-2-sweep" | "phase-3-trace" | ... | "done",
  "state":  "running" | "complete" | "failed",
  "seeds": "42 99 7 2025 314",
  "reps": "3",
  "duration": "900",
  "parallel": "4",
  "allow_dirty": "0",
  "note": "<optional free-text>"
}
```

**Exit codes:** `0` success · `2` hard fail (dirty tree without `ALLOW_DIRTY=1`, cargo build failed, disk write failed). Survival/continuity canary regressions are **recorded but do not halt the run** — collect-everything failure mode.

## Wall-clock cost

Default config (5 seeds × 3 reps × 900s + 5×2 focal traces × 900s + 2 conditional variants × 900s) ≈ **4–6 hours wall time** at `PARALLEL=4`. Smoke first with `SEEDS="42" REPS=1 DURATION=60`.

## Environment overrides (no flags — env-vars only)

| Variable         | Default              | Purpose                                                                  |
|------------------|----------------------|--------------------------------------------------------------------------|
| `SEEDS`          | `"42 99 7 2025 314"` | Whitespace-separated seed list.                                          |
| `REPS`           | `3`                  | Sweep reps per seed.                                                     |
| `DURATION`       | `900`                | Seconds per long soak.                                                   |
| `PROBE_DURATION` | `60`                 | Phase 1 smoke duration per seed.                                         |
| `PARALLEL`       | `4`                  | xargs concurrency for Phase 2/3.                                         |
| `ALLOW_DIRTY`    | `0`                  | Set `1` to permit `commit_dirty=true` headers (NOT for shipping baselines). |
| `SKIP_PHASE_4`   | `0`                  | Set `1` to skip fog/storm conditional runs.                              |
| `ROOT`           | `logs`               | Output root (rare to override).                                          |

## Examples

```bash
# Production baseline.
just baseline-dataset 2026-04-25

# Smoke test (≈ 4 min total).
SEEDS="42" REPS=1 DURATION=60 PROBE_DURATION=10 just baseline-dataset smoke

# Skip the conditional-weather phase (saves ~20 min).
SKIP_PHASE_4=1 just baseline-dataset 2026-04-25

# Resume a partial run — already-complete phases skip automatically.
just baseline-dataset 2026-04-25
```

## After completion — the promote step

`baseline-dataset` does NOT activate the new baseline. After it completes, run:

```bash
just promote logs/baseline-<LABEL> <LABEL>
```

This writes `logs/baselines/current.json` pointing at the new directory; subsequent `just verdict` calls read that file to resolve the comparison baseline.

## Polling progress

The `STATUS.json` mirror is designed for `watch -n 5 jq '.phase, .state, .updated_at' logs/baseline-<LABEL>/STATUS.json` — the run produces no other live signal. Phase 2/3 also produce per-run `events.jsonl` files that can be tailed individually.

## Hook safety

`logs/baseline-<LABEL>/` is protected by `.claude/hooks/no-log-overwrite.py`. The orchestrator refuses to overwrite an existing baseline directory in the canonical path — rename or delete the old one first if you intend to rebuild under the same label.

## Non-goals

- Does not run `just verdict` against the new baseline. Run it manually after `promote` to confirm the new baseline produces a clean verdict on a fresh seed-42 soak.
- Does not edit `docs/balance/healthy-colony.md`. The REPORT.md is informational; promoting healthy-colony fingerprint values is a separate manual edit.
- Does not commit anything. Baseline directories are git-ignored; the only artifact you commit is `logs/baselines/current.json` (after `just promote`).
