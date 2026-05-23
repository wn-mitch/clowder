---
id: 459
title: Retire author_joint_intentions per-tick hot path
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: []
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`clowder::ai::joint_intention::author_joint_intentions` consumes **39.86%
self-time / 40.36% inclusive** in a 60-second seed-42 flamegraph
captured 2026-05-23. That is the same anti-pattern memory
`project_per_tick_discipline_default_event_driven` warns about and that
ticket 431 closed for `passive_familiarity`: a per-tick system doing
state-accumulation work that should be event-driven (firing on the
mutation events that change the underlying state instead of resampling
it every frame).

Captured during the ticket-004 perf investigation (after the magic gate
retirement) — the symbol was already hot at the parent commit and is
unrelated to ticket 004's substrate change. The 2.2× duration slowdown
observed in 004's verdict (60 ticks/sec vs the 095-phase-1a-shadow
baseline's 132 ticks/sec) is dominated by this symbol, not by 004's
six-extra-DSE-evaluations-per-cat.

## Scope

- Audit `src/ai/joint_intention.rs::author_joint_intentions` to identify
  what state it accumulates and which underlying events should drive it.
- Migrate authoring to fire on the relevant Bevy `Message` (likely
  `CatMoved`, `CatAcquiredIntention`, `BondFormed`, or similar) against
  cached state.
- Preserve seed-42 determinism — if the system relies on iteration order
  of a `BTreeMap`-like structure for tie-breaking, preserve it on any
  swap (per ticket 431's load-bearing iteration-order discipline).
- Validate via flamegraph: the symbol's inclusive% should drop to <5%.
- Validate via soak: per-tick rate (`ticks / wall_seconds`) should
  recover materially (target: match or beat the 095-phase-1a-shadow
  baseline at 132 ticks/sec on the canonical seed-42 deep-soak).

## Out of scope

- JointIntention semantics or substrate shape (covered by ticket 127 and
  its follow-ons).
- Other per-tick perf bottlenecks that may surface after this one
  retires.

## Current state

Ticket 127 (Commits A/B/C, landed) introduced `JointIntention` substrate
for two-cat practices. The authoring system was wired in per-tick at the
time. The flamegraph captured during 004's verification confirms it has
become a dominant cost. Ticket 431 closed the parallel `passive_familiarity`
case with a `CatMoved`-driven `NearPairCache`; the same shape should apply
here.

## Approach

1. Read `src/ai/joint_intention.rs` end-to-end. Identify the state being
   accumulated per tick and the events that should drive each accumulator.
2. Cross-reference with ticket 431's `NearPairCache` shape — likely a
   reusable pattern.
3. Refactor `author_joint_intentions` from a per-tick system to a
   message-driven one. Cache the accumulated state in a Resource.
4. Add a debug-only invariant assertion against the pre-cache state to
   localize divergences at the first divergent tick (per the seed-determinism
   trap memory `project_per_tick_discipline_default_event_driven` calls out).
5. Re-run `just flamegraph 42 60` and confirm the symbol drops to <5%.
6. Run `just soak-trace 42 Simba` + `just verdict` and confirm per-tick
   rate matches baseline + survival/continuity canaries hold.

## Verification

1. `just check` / `just test` — type / linter / unit-test gates.
2. `just flamegraph 42 60` — `author_joint_intentions` < 5% inclusive.
3. `just soak-trace 42 Simba` — per-tick rate ≥ 100 ticks/sec (target 132).
4. `just verdict logs/tuned-42-<sha>/` — survival hard gates pass,
   continuity canaries fire ≥ 1 each, no new "fail" bands in
   characteristic drift.
5. `just frame-diff <baseline> <new>` — no per-DSE drift attributable
   to the change.

## Log

- 2026-05-23: opened from ticket 004's perf investigation. Flamegraph
  evidence: `author_joint_intentions` at 39.86% self / 40.36% inclusive
  on seed 42, parent commit cf6f36f5. Parallel precedent: ticket 431's
  passive_familiarity retirement.
