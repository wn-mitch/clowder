---
id: 396
title: verdict canary for new high-rate plan_failures_by_reason keys
status: done
cluster: diagnostics
initiative: []
orchestration: swarm-safe
added: 2026-05-16
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 886cd1f7b60a
landed-on: 2026-05-17
---

## Why

User-flagged during ticket 394 Phase A investigation
(2026-05-16): *"we should add those as failing canaries if it's
logarithmically higher rate wise"*. Currently `verdict`'s footer
drift check operates only on top-level footer fields; nested dicts
like `plan_failures_by_reason` are invisible. The 394 regression
(2439 Wean failures vs 0 in baseline, ~0.02/tick) wouldn't have
been caught by any existing canary — the colony absorbed the
churn (no deaths, welfare improved) so survival + continuity gates
both passed, but the substrate was visibly dirty.

A plan-failure rate canary would catch this class of regression:
when any plan_failures_by_reason key has rate ≥ 10× the baseline
rate, OR is new vs baseline (baseline rate = 0) with rate above
some absolute threshold (~0.005/tick = 1 per 200 ticks), fail the
verdict with the key + rate + ratio surfaced in `next_steps`.

## Scope

- New canary in `scripts/verdict.py` (or wherever the Python
  verdict orchestrator lives — `just verdict` recipe wraps it).
- Iterates `plan_failures_by_reason` and `planning_failures_by_reason`
  (and possibly `interrupts_by_reason`) in the footer.
- For each key:
  - If baseline has the key with count > 0: rate ratio =
    observed_rate / baseline_rate. If ratio ≥ 10 and observed_rate
    above some absolute floor (e.g., 0.001/tick), flag.
  - If baseline doesn't have the key (new key) and observed_rate
    above some absolute threshold (e.g., 0.005/tick = 1 per 200
    ticks), flag.
- Verdict envelope's `next_steps` includes a `just q footer` drill
  command for each flagged key.

## Out of scope

- Backfilling the canary against historical regressions. Land
  forward-only.
- Specific threshold tuning. Start with 10× / 0.005/tick defaults;
  iterate.
- Hooking this into `just bisect-canary` (separate ticket if needed).

## Verification

- Synthetic test: a fake footer with Wean: 2439 vs baseline Wean: 0
  should trigger the canary.
- Run `just verdict logs/tuned-42-394-r11 --baseline
  logs/tuned-42-d633bcc5/events.jsonl` after this lands; should
  flag the 9-event Wean failure if it survives R11 + R13 (it
  shouldn't, given the absolute threshold), but should flag any
  similar churn pattern.
- Run against `logs/tuned-42-attempt11` (the original 2439 Wean
  failures); should flag clearly.

## Log

- 2026-05-16: opened as follow-on from ticket 394 Phase A. The
  user's intuition that "logarithmically higher plan-failure rates
  should be a failing canary" is the design seed.
- 2026-05-17: added plan_failure_canary() to scripts/verdict.py iterating plan_failures_by_reason + planning_failures_by_reason + interrupts_by_reason; thresholds 10x ratio + 0.001/tick floor + 0.005/tick new-key floor; 19 unit tests; replay against logs/tuned-42-attempt11 flagged Wean 0→2439 (0.0195/tick) as new-high-rate. Bundled with CLAUDE.md design pillar #4 enshrining commitment-via-pin as override (precedent: 364→397 kitten-arc cluster).
