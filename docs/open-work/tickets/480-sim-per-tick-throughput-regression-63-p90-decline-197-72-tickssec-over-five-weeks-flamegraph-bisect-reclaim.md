---
id: 480
title: Sim per-tick throughput regression — ~63% p90 decline (197->72 ticks/sec) over five weeks; flamegraph-bisect + reclaim
status: ready
cluster: ai-substrate
initiative: []
added: 2026-05-27
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Sim per-tick throughput has fallen ~63% over five weeks. On fixed-duration
seed-42 soaks (`just soak` = 900 s wall-clock), p90 ticks/sec dropped from ~197
(week of 04-20) to ~72 (week of 05-25), monotonically. p90 (best-case,
uncontended runs) is used deliberately — it controls for the parallel-session
CPU contention that muddies the daily average, so this is real per-tick cost
growth, not a busier machine. The user-visible symptom is shrinking
`seasons_survived`: the same 900 s now covers ~3 sim-seasons instead of ~4.3,
not because colonies die sooner (they don't — soaks end healthy) but because
fewer ticks fit in the window. A performance pass during this period bent the
curve but did not reverse it; the month's feature work (wildlife GOAP cutover,
crafting aspirations, body-zones, belief integrator, HTN methods, festering)
added per-tick cost faster than it was reclaimed.

## Scope
- Flamegraph-bisect to localize the dominant new per-tick cost since early May.
- Reclaim throughput toward the early-May p90 (~150+ ticks/sec) without
  regressing behavior (seed-42 verdict hard gates + continuity canaries hold).
- Where the hot frame is an O(N^2) sweep or per-tick recompute that should be
  event-driven + cached, retire it per the "default to event-driven, justify
  per-tick" discipline (memory `project_per_tick_discipline_default_event_driven`).

## Out of scope
- Behavior / balance changes. A throughput fix must be behavior-preserving
  (a refactor that changes sim behavior is a balance change — out of scope here).
- The throughput measurement tooling itself — the `soak-throughput-over-time`
  logdb chart recipe already landed (diagnostics commit, 2026-05-27) and is the
  instrument this ticket reads.

## Current state
Measurement landed: `just logdb-chart soak-throughput-over-time --seed 42`
renders ticks/sec per soak with a LOESS trend over the full archive history.
The steepest segment of the cliff is between the week of 05-11 (p90 ~150) and
05-25 (p90 ~72) — the tightest window to bisect. Two existing open tickets name
specific per-tick hot paths and should be folded into this investigation rather
than duplicated:
- **205** — `social_status_distress` perception cost (~25% per-tick slowdown
  from an O(N^2) pass).
- **459** — retire `author_joint_intentions` per-tick hot path.
Precedent for the methodology + fixes: **431** (`passive_familiarity` at 64%
inclusive CPU, retired behind a `CatMoved`-driven cache) and **427** (per-tick
allocation hotspots — scratch-buffer reuse).

## Approach
1. `cargo flamegraph` on a short headless run at HEAD, and again at an early-May
   commit (e.g. the week-of-05-04 peak), then diff the hot frames. Per memory
   `feedback_perf_refactor_needs_flamegraph`: asymptotic/aggregate reasoning
   misled us on ticket 205's first attempt — flamegraph before and after, and
   redesign if the hot frame moves unexpectedly.
2. Rank new/grown frames. Prime suspects by landing date inside the cliff
   window: 463's per-tick crafting-aspiration scoring (`CraftItemAspiration` +
   `CatRecentCrafts` + picker recipe scoring, landed 05-26 — the day the daily
   average dropped to 62.6), the belief integrator, festering per-tick
   observation, and the wildlife-GOAP per-tick branches.
3. For each dominant frame, apply the event-driven-with-cache pattern (431) or
   allocation reuse (427); preserve `BTreeMap`/iteration order where it is
   load-bearing for seed determinism (the 431 trap), gating any swap with a
   debug-only invariant assertion against the pre-cache result.

## Verification
- `just logdb-chart soak-throughput-over-time --seed 42` — the LOESS trend
  turns back up; p90 recovers toward the early-May band.
- Per-fix: flamegraph confirms the targeted frame shrank; `just soak 42` +
  `just verdict logs/tuned-42` hold all hard survival gates + continuity
  canaries (behavior-preserving).
- `just check && just test`.

## Log
- 2026-05-27: opened from the 477-landing session's throughput investigation.
  Finding: p90 ticks/sec 197 -> 72 over five weeks (logdb cross-run query +
  new `soak-throughput-over-time` chart). Bisect window 05-11 -> 05-25; fold in
  205 + 459; methodology per 431 / 427.
