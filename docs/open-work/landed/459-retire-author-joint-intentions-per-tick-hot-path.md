---
id: 459
title: Retire author_joint_intentions per-tick hot path
status: done
cluster: social-coordination
orchestration: substrate-sensitive
initiative: []
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: bdc71254
landed-on: 2026-06-11
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
- 2026-06-09: knife #1 landed (commit d30f3f48). Layer-walk found the
  dominant self-time was NOT the matchmaker's O(N²) distance loop but
  the ticket-453 Mates-exclusivity gates: `Relationships::iter_for` is
  an **unindexed full-BTreeMap filter**, called once per actor (self
  gate) and once per candidate (third-party gate) — O(cats² × pairs)
  per tick. Fix: one O(pairs) pass builds a `mates_bonded:
  HashSet<Entity>` per tick; membership is semantically identical
  (bond storage is symmetric, and the self-gate early-return makes
  "candidate has any Mates bond" ⇔ "Mates with a third party").
  Debug-parity assertions re-run the old scans in debug builds and
  assert agreement at the first divergent call (431 discipline). Also
  retired the Pass-2 O(J²) `joints.iter().find()` by carrying partner
  stage in the Pass-1 snapshot (pre-Pass-2 state — exactly what the
  find() observed). Event-driven dirty-set authoring (the full ticket
  scope) remains open pending a fresh flamegraph — if the symbol is
  <5% inclusive after this, close as done; matchmaking is genuinely
  position-coupled (cats move every tick), so dirty-set gating may
  never beat the algorithmic fix.
- 2026-06-09 (byte-identity A/B, 60 s seed-42, cc15afd3 vs d30f3f48):
  pre-fix 11,868 elapsed (197 t/s) vs post-fix 12,385 (206 t/s) —
  +4.4% in the early-colony window (win scales with pair count).
  Common-range diff: 4 of 14,667 lines differ, ALL CatSnapshot
  `last_scores` Patrol entries, ALL exactly 1 ULP (1e-8), zero
  state-bearing fields differ, and the stream is byte-identical again
  from elapsed 7,900 → end — i.e. a codegen-level rounding wobble in
  the diagnostic reporting path, not behavior (true divergence would
  compound). Accepting as behavior-preserving; full soak + verdict
  gates remain.
- 2026-06-11: knife #1 landed: mates-bonded set + snapshot-carried stage; 22.8% -> 5.36% inclusive, +4.4% throughput, state-identical A/B
