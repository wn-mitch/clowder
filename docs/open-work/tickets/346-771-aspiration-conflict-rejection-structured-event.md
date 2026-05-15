---
id: 346
title: §7.7.1 aspiration conflict-rejection structured event
status: parked
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-14
parked: 2026-05-14
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 056 landed the §7.7.1 hard-conflict adoption gate
(`can_adopt` in `src/ai/aspirations/mod.rs`). Today rejection is
**silent** — the candidate is skipped at the second-slot loop in
`check_second_aspiration_slot` with no narrative emission and no
structured event in `events.jsonl`. The project's "substrate over
hacks" pillar tilts toward making the gate's choice visible (the L2
trace should explain decisions), but emitting per-rejection telemetry
without a consumer would be log spam during sweeps.

Parked until a balance investigation surfaces "I needed to know why
chain X wasn't adopted." When that happens, this ticket unparks.

## Scope (when this unparks)

- Define a structured message: `AspirationConflictRejected { cat,
  candidate, blocker, class }` (or similar shape — the right
  encoding depends on which consumer is asking).
- Emit at the gate site in `check_second_aspiration_slot` (one or two
  sites if `select_aspirations` also runs the gate in future).
- Route through `NarrativeTier::Debug` if narrative-style; or through
  `MessageWriter<...>` if event-style. Decide based on consumer needs.
- Enroll in `Feature::expected_to_fire_per_soak()` only if the
  rejection is expected to fire ≥1× per seed-42 deep-soak (depends on
  how often the 2 hard pairs actually collide during second-slot
  scoring).

## Out of scope

- Reworking the gate itself (056 landed it).
- Soft-class telemetry (ticket 345).
- Per-arc valence-target reads (ticket 344).

## Current state

Parked at open time. The 056 land's soak verification will surface
whether rejection fires often enough in production to warrant
observability — that's the natural unparking trigger.

## Approach

Deferred. Encoding depends on the consumer (focal-trace visualizer,
sweep-stats query, narrative log, etc.).

## Verification

Deferred until unparking. Will likely include a unit test asserting
the message fires at the rejection site, plus a `just q` recipe in
`docs/diagnostics/log-queries.md` for the new message shape.

## Related work

- 056 — sister ticket; landed the silent gate this ticket would make
  observable.
- 344 — sister ticket; per-arc valence targets (different consumer).
- 345 — sister ticket; soft-class weights (different concern).

## Log

- 2026-05-14: opened parked as a 056 follow-on (split-out per
  CLAUDE.md antipattern-migration discipline). Unparks when a
  balance investigation surfaces the need for rejection telemetry.
