---
id: 434
title: 431 re-baseline Stage A's perturbation cost against correct archives
status: ready
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
initiative: []
added: 2026-05-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 431's mid-flight misdiagnosis (Stages A and B compared via mis-labeled archives — see the 431 §"Stage B drift resolution" section) means the Stage A schedule-edge perturbation cost was never cleanly attributed. The drift the previous session attributed to Stage B was actually Stage A's perturbation vs pre-Stage-A. Stage H's tooling now prevents the misdiagnosis from recurring (`just frame-diff` hard-errors on cross-commit; `_check-binary-fresh` refuses stale binaries; archive directory naming embeds the commit). This ticket actually does the clean re-baselining under Stage H's protections so future stages have a known perturbation budget to compare against.

## Scope

- Run a fresh `just soak-trace 42 Simba` against the canonical pre-Stage-A binary (`976e8e0c` — the docs-backfill commit; renamed archive at `logs/431-pre-stage-a-83b65904/` is close but a fresh run under Stage H produces the canonical-named archive `logs/tuned-42-976e8e0c/` directly).
- Run `just soak-trace 42 Simba` against Stage A's actual binary (`f4047e2f`) — archive at `logs/tuned-42-f4047e2f/`.
- Run `just soak-trace 42 Simba` against Stage B's actual binary (`4b670a6c`) — archive at `logs/tuned-42-4b670a6c/`.
- `just frame-diff` Pre-A → A — that delta IS Stage A's schedule-edge perturbation (now exits 2 unless `--allow-cross-commit`; pass the flag).
- `just frame-diff` A → B with `--allow-cross-commit` — that delta IS Stage B's pure substrate-swap cost (expected: byte-clean within the survival/continuity gates, per the debug-only cache-vs-brute-force assertion).
- `just verdict` against each archive vs the promoted baseline — confirms all three pass the survival + continuity gates.

## Out of scope

- Stage C's rescoped design (lives in ticket 432).
- Stage F's cross-system snapshot dedupe (lives in ticket 433).
- Promoting any of these as the new canonical baseline — that decision goes to a separate balance-doc session.

## Current state

Opened 2026-05-20 alongside the 431 closeout. Blocked by 431 lands (which carries Stage H's tooling — without it, the soak gate doesn't refuse stale binaries and the misdiagnosis class isn't structurally closed). The actual re-baselining work is short (three 15-min soaks + frame-diffs); the value is the clean reference data for Stages C / D / G follow-on perf attribution.

## Approach

Straightforward script-driven verification:
1. `jj edit 976e8e0c` → `cargo build --release` → `just soak-trace 42 Simba` → archive lives at `logs/tuned-42-976e8e0c/`.
2. Same for `f4047e2f`, `4b670a6c`, and HEAD (= 431's final commit; the current Stage G commit on land).
3. Run the three frame-diffs with `--allow-cross-commit` (the diffs are intentional cross-commit; the flag acknowledges that).
4. Document the Stage A perturbation magnitude in the 431 landed/ file's Log.

## Verification

- Each archive passes `just verdict` (survival + continuity canaries hold).
- Frame-diffs produce a numeric Stage A perturbation magnitude attributable to the schedule-edge cost.
- Frame-diff Stage A → Stage B is near-zero (verifies cache logic is byte-clean in release as well as debug).

## Log

- 2026-05-20: opened as a 431 closeout follow-on to finally re-baseline Stage A's perturbation cost against the corrected archives. Becomes the reference dataset for Stages C and F follow-on perf attribution.
