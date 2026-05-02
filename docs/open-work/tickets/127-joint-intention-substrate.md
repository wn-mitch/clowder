---
id: 127
title: Joint-intention substrate for two-cat practices
status: blocked
cluster: C
added: 2026-05-02
parked: null
blocked-by: [126]
supersedes: []
related-systems: [ai-substrate-refactor.md, scoring-layer-second-order.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Spun out of 126 (`## Out of scope`). 126 lands per-cat perceivable
`HeldIntention` substrate. Versu-style practices (007 cluster-C C2)
need *joint*-commitment semantics: courtship, co-mentoring, and
joint cache-stocking are not "each cat independently scoring the
same DSE" — they are *one* multi-stage structure that two cats
co-enter, co-progress, and that drops cascade across when one party
abandons.

The shape this ticket has to commit:

- A `JointIntention` abstraction (or a `HeldIntention`-pair
  invariant) that ties two cats' commitments together such that
  drop on one side propagates to the other within a bounded number
  of ticks.
- Compatibility predicate at adoption time (both cats must hold
  *compatible* intentions, not identical — courter-courtee roles
  differ).
- Stage progression vocabulary (greeting → display → consummation
  → bond) that survives across `should_drop_intention` cycles.
- Drop cascade with a `DropReason` that names which side dropped
  first, so post-hoc inspection (`just inspect <cat>`) can
  attribute social-fabric tears correctly.

Not in scope here; this is the placeholder for the design.

## Dependencies

- Blocked by 126 (`HeldIntention` substrate must land first).
- Pairs with 027 / 027b (mating cadence) — `PairingActivity` is
  the existing two-cat-pairing pattern this ticket would
  generalize across non-mating practices.
- Cluster role: C2 in 007.

## Log

- 2026-05-02: opened as 126 follow-on per CLAUDE.md
  antipattern-migration rule.
