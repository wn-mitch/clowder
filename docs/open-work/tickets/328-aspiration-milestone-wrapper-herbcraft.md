---
id: 328
title: Herbcraft aspiration_milestone_wrapper + emits tables
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [aspiration_milestone_wrapper.herbcraft]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 chain follow-on. Authors the Herbcraft chain's
`Milestone.emits[]` tables and flips
`aspiration_milestone_wrapper.herbcraft` from PendingSubstrate to
Live.

## Scope

- Author `Milestone.emits[]` for every milestone of the Herbcraft
  chain in `src/systems/aspirations.rs`.
- Flip `aspiration_milestone_wrapper.herbcraft` to Live in
  `populate_method_registry`.

## Out of scope

- Tuning emit priorities via balance soak.
- New Herbcraft-domain DSE substrate; this ticket wraps existing
  GatherHerbs / PrepareRemedy / SetWard primitives.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #10 of 25, blocked on #319 + #321. Batch C — 5-way parallel
with #327 / #329-#331.

## Approach

Per htn-methods.md §H + Tier-1/Tier-2 split. Same shape as #325
Hunting wrapper; differs in chain-specific milestones and emit
levers.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a cat with Herbcraft aspiration
  shows L1Aspiration emit-walks with non-empty rows.

## Log

- 2026-05-14: opened as 128 epic child #10 (Batch C chain
  follow-on).
