---
id: 031
title: Balance-tooling composition layer (verdict / hypothesize / sweep-stats / fingerprint / explain / bisect-canary / promote)
status: done
cluster: null
landed-at: null
landed-on: 2026-04-26
---

# Balance-tooling composition layer (verdict / hypothesize / sweep-stats / fingerprint / explain / bisect-canary / promote)

**Landed:** 2026-04-26 | **Ticket:** 031.


## Why

Clowder's data-collection surface is unusually rich (canary scripts, sweeps, focal traces, parameterized log queries, structured 17-field footer, five subagents) but has no **composition layer**. Common workflows — "is this run OK?", "is this hypothesis worth shipping?", "is this delta significant?" — require chaining 3+ scripts and eyeballing 3+ outputs. For Claude Code agentic loops this is a multi-turn cost that should be a single-call verdict.

This ticket builds the composition layer (Tiers 1–4 of the gap analysis at `~/.claude/plans/what-are-some-gaps-melodic-kay.md`) and **retires superseded tools** (autoloop, score-track, score-diff, balance-report, sweep_compare.py, analyze_*.py) so future agents reach for the new surface.

## Scope

**Phase A — verdict glue + retirements:**
- `scripts/verdict.py` + `just verdict <run-dir>` — one-call run validation. Replaces `autoloop`.
- `scripts/sweep_stats.py` + `just sweep-stats <dir> [--vs <baseline>]` — per-metric mean/stdev/CI95/Welch-t/effect-size. Replaces `balance-report`, `score-diff`, `sweep_compare.py`.
- `SimConstants::from_env()` env-override hook + header echo of applied overrides.
- `scripts/hypothesize.py` + `just hypothesize <yaml>` — formalizes the four-artifact balance methodology.
- Delete: `autoloop`, `score-track`, `score-diff`, `balance-report`, `sweep_compare.py`, `analyze_eat_threshold.py`, `analyze_emergent.py`, `analyze_score_competition.py`.

**Phase B — onboarding:**
- `docs/balance/healthy-colony.md` (per-metric expected ranges + meaning).
- `scripts/fingerprint.py` + `just fingerprint <run-dir>`.
- `scripts/explain_constant.py` + `just explain <constant-path>`.
- Wire fingerprint bands into `verdict`'s footer-drift output.

**Phase C — regression:**
- `scripts/bisect_canary.sh` + `just bisect-canary <metric> <bad-sha>`.
- `scripts/promote.sh` + `just promote <run-dir> <label>` + `logs/baselines/<label>.json` registry.

**Phase D — instrumentation:**
- Per-cat fulfillment axis footer fields (mean/stdev/min/max for acceptance / mastery / purpose / respect + aggregate).
- `scripts/build_sensitivity_map.sh` + `logs/sensitivity-map.json`. Wire into `explain`.

## Out of scope

- Multi-game/multi-project tooling.
- Web dashboards / live UI.
- Replacing primitives (`check-canaries`, `check-continuity`, `q`, `frame-diff`, `sweep`, `soak`, `soak-trace` are kept and wrapped).

## Current state

Plan file: `~/.claude/plans/what-are-some-gaps-melodic-kay.md`. Auto-mode build authorized 2026-04-26.

## Approach

Each new tool follows the Claude Code turn pattern: one bash invocation, structured JSON to stdout, exit code reflects verdict, `next_steps` hint embedded, background-safe for long-runners. Phases A–D land sequentially; Phase A bundles all retirements alongside their replacements so an agent never sees both surfaces.

## Verification

- `just check` + `just test` pass at every phase commit.
- Smoke: `just verdict logs/<existing-run>` emits valid JSON with verdict and next_steps.
- End-to-end: `just hypothesize` on a tiny example produces baseline + treatment sweeps + draft balance doc; `just verdict` on the treatment run agrees with hypothesize's concordance call.
- Retirement test: grep repo for retired tool names — zero hits outside `docs/open-work/landed/`.
- Subagent integration: spawn an Explore agent with the new command; it acts on the JSON envelope without re-reading source.

## Log

- 2026-04-26: Ticket opened, all four phases landed in one auto-mode session.
  - **Phase A** — verdict + sweep-stats + hypothesize, plus retirement of `autoloop`, `score-track`, `score-diff`, `balance-report`, `sweep_compare.py`, `analyze_*.py`. `SimConstants::from_env()` env-override hook + header echo of applied overrides. End-to-end smoke confirmed (CLOWDER_OVERRIDES applied, header echoes patch, hypothesize ran baseline + treatment + drafted balance doc).
  - **Phase B** — `docs/balance/healthy-colony.md` + `fingerprint.py` + `explain_constant.py`. Bands derived from `logs/sweep-baseline-5b/` (15 runs).
  - **Phase C** — `bisect_canary.sh` + `promote.sh` + `logs/baselines/<label>.json` registry. `.gitignore` updated so the registry is tracked while individual log dirs stay ignored.
  - **Phase D** — welfare-axis footer fields (`welfare_axes` block: social_warmth + Maslow acceptance/respect/mastery/purpose, each with mean/stdev/min/max). `build_sensitivity_map.sh` + `build_sensitivity_map.py` for the per-knob → metric Spearman map (run on a quiet weekend; output committed to `logs/sensitivity-map.json` and read by `just explain`).
  - Ticket spans four commits on `wnmitch/031-balance-tooling`. Concurrent ticket-027 session in the same default workspace caused two transient jj rebase shuffles that were reset cleanly each time.


---
