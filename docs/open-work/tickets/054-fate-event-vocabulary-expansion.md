---
id: 054
title: §7.7.c Fate event-vocabulary expansion — Calling, destiny, fated-pair convergence
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, the-calling.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Today's `src/systems/fate.rs` emits only `FatedLove` / `FatedRival`.
Aspirations that should respond to the Calling, destiny modifiers, or
fated-pair convergence need those events to exist.

## Scope

- New event variants on the Fate axis: at minimum `CallingStirred`,
  `DestinyMomentum` (or analogous; spec under-specified here),
  `FatedPairConverged`.
- §7.7.c aspiration consumers wired to react to each.
- Cross-link with the Calling subsystem doc as that work matures.

## Gating

The Calling subsystem itself is documented at
`docs/systems/the-calling.md` but has no implementation yet and no
current open-work ticket. Any worker picking this up should first
scope a Calling subsystem ticket (or confirm the design is adequate
to ship `CallingStirred` as an event without the full subsystem) —
that's a precondition decision, not a `blocked-by`.

The `the-calling.md` doc is rank 3 in
`docs/systems-backlog-ranking.md` (cross-cutting debt; lands alongside
Calling implementation).

## Out of scope

- The full Calling subsystem implementation. This ticket commits the
  *event vocabulary* shape; the subsystem that *fires* most of these
  events is its own scope.

## Approach

Two routes. Route A: open a Calling subsystem ticket first, gate this
on it. Route B: ship the event variants empty-bodied so consumers can
be wired now and the Calling system fills them in later. Route A is
cleaner; Route B is faster but accumulates ghost events.

## Verification

- Unit tests on each new event variant.
- Integration test: §7.7.c aspiration consumer fires when its target
  event fires.
- `just check` green.
- Soak verdict: depends on which events are wired and whether the
  Calling subsystem fires any in the soak window.

## Log

- 2026-04-27: opened from ticket 013 retirement (spec-follow-on debts
  umbrella decomposition). Original sub-task 13.3 in spec
  `docs/systems/ai-substrate-refactor.md` §7.7.c.
