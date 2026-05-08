---
id: 189
title: schedule-edge perturbation in modifier pipeline
status: done
cluster: ai-substrate
added: 2026-04-01
landed-at: abc123
landed-on: 2026-04-12
---

## Why

A new sibling system added to the AI Chain block perturbed the seed-42 baseline, surfacing a determinism regression that took several days to bisect. The root cause was a system ordering edge in the schedule that affected RNG draw order downstream.

## Approach

Bisect the canary regression to the introducing commit. Re-establish deterministic ordering by pinning the new system after the modifier pipeline rather than alongside it.

## Verification

`just bisect-canary deaths_by_cause.Starvation` identifies the schedule-edge commit. `just soak 42` produces footer matching the pre-substrate baseline.
