---
id: 053
title: §7.7.b death-event grief emission — relationship-classified survivors payload
status: blocked
cluster: null
added: 2026-04-27
parked: null
blocked-by: [007]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Today's `src/systems/death.rs` emits only generic-proximity grief plus
`FatedLove` / `FatedRival` removal. §7.7 aspirations need a richer event
so per-relationship reconsideration filters can distinguish
grief-for-mate vs. grief-for-mentor vs. grief-for-kin.

Candidate event shape per spec: `CatDied { cause, deceased,
survivors_by_relationship }` (or equivalent), feeding §7.7.b
relationship-classified reconsideration triggers.

## Scope

- Replace today's generic grief emission with a relationship-classified
  payload event.
- Wire §7.7.b reconsideration consumers to filter on the new payload.
- Migrate `FatedLove` / `FatedRival` removal to use the same event
  rather than a separate path.

## Out of scope

- Formal multi-tier relationship modeling beyond the current three-tier
  `BondType` (that's the C3 belief modeling work in cluster C / ticket
  007 — this ticket consumes whatever 007 produces).

## Approach

After 007's C3 belief modeling lands, the relationship taxonomy becomes
sufficient for the survivors classification. Until then, this ticket
stays blocked.

## Verification

- Unit tests on payload shape: each death emits one event with the
  correct survivors-by-relationship breakdown.
- Integration test: kitten dies → mother's grief-for-kin reconsideration
  triggers; mate dies → grief-for-mate triggers; both don't cross-fire.
- `just check` green.
- Soak verdict: behavior shift expected (richer reconsideration
  triggering); document hypothesis per CLAUDE.md balance methodology.

## Log

- 2026-04-27: opened from ticket 013 retirement (spec-follow-on debts
  umbrella decomposition). Original sub-task 13.2 in spec
  `docs/systems/ai-substrate-refactor.md` §7.7.b.
