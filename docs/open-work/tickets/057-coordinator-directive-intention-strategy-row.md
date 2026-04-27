---
id: 057
title: §7.3 coordinator-directive Intention strategy row — `SingleMinded` with override
status: blocked
cluster: null
added: 2026-04-27
parked: null
blocked-by: [007]
supersedes: []
related-systems: [ai-substrate-refactor.md, strategist-coordinator.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Spec §7.3 footer commits `SingleMinded` Intention-strategy with a
coordinator-cancel override for coordinator-directive aspirations.
Today the row's content is unfilled because the coordinator DSE
itself doesn't exist; it lands with C4 (cluster C / ticket 007's
strategist-coordinator track).

## Scope

- Add the §7.3 row's `SingleMinded` strategy entry with a
  coordinator-cancel override clause.
- Wire the override path through the §7.2 commitment evaluator so
  coordinator directives can interrupt `SingleMinded` commitments.
- Resolve the ledger-level pointer in
  `docs/systems/ai-substrate-refactor.md` once landed.

## Out of scope

- The coordinator DSE itself (that's C4 in cluster C / ticket 007).
- Other §7.3 row finalizations.

## Approach

When 007's C4 strategist-coordinator track lands, wire this row in
the same PR. Mechanically: extend
`src/ai/intention.rs::IntentionStrategy` (or wherever the
strategy-row table lives) with the coordinator-cancel variant; thread
through the §7.2 commitment evaluator.

## Verification

- Unit test: a cat under `SingleMinded` commitment to disposition X
  receives a coordinator directive Y; commitment yields to Y.
- Unit test: a cat under `SingleMinded` commitment to disposition X
  without a coordinator directive holds.
- `just check` green.
- Soak verdict: behavior-neutral on canonical seed-42 (no coordinator
  directives fire absent C4 implementation); becomes behavior-active
  once coordinator DSE lands.

## Log

- 2026-04-27: opened from ticket 013 retirement (spec-follow-on debts
  umbrella decomposition). Original sub-task 13.6 in spec
  `docs/systems/ai-substrate-refactor.md` §7.3 footer note.
