---
id: 129
title: Care DSEs over perceivable intentions
status: blocked
cluster: C
added: 2026-05-02
parked: null
blocked-by: [126]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Spun out of 126 (`## Out of scope`). 126 makes per-cat
`HeldIntention` visible to other cats' DSEs via standard Bevy
queries. The helper-side consumer DSEs that *read* other cats'
intentions and adopt their own intentions in response — "Hazel
intends to rest because injured → I form an intention to make her
soup" — are the next layer up.

The shape this ticket has to commit:

- A `Caretake_target` (or named per care archetype: `Comfort`,
  `Provision`, `Defend`) target-taking DSE following the §6
  pattern, whose candidate query includes cats holding
  `HeldIntention { intention: Goal { state: rest, .. }, .. }`
  alongside `Injured`/`LowHealth`/`HungryAndImmobile`/etc.
  markers.
- Care-task HTN composition (or, pre-128, a hand-authored
  decomposition): "make soup for Hazel" → `[forage-ingredients,
  return-to-firepit, cook, deliver]`.
- Soft-claim primitive against the care *target* (not just the
  ingredients) so two cats don't both make soup for the same
  injured cat. Reuses 080's `Reserved` pattern at goal granularity.
- Helper personality bias (compassion / kin / bond) on the
  `Caretake_target` weight curve so role differentiation emerges
  without a director.

Not in scope here; this is the placeholder for the design.

## Dependencies

- Blocked by 126 (`HeldIntention` perceivability is the
  prerequisite).
- Pairs with 080 (Reserved soft-claim primitive — extend to
  goal-level claims).
- Pairs with 128 (HTN methods would author the care decomposition
  cleanly, but this ticket can ship pre-128 with a
  hand-authored sequence).

## Log

- 2026-05-02: opened as 126 follow-on per CLAUDE.md
  antipattern-migration rule.
