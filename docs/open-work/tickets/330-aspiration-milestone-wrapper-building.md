---
id: 330
title: Building aspiration_milestone_wrapper + emits tables
status: ready
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [aspiration_milestone_wrapper.building]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 chain follow-on. Authors the Building chain's
`Milestone.emits[]` tables and flips
`aspiration_milestone_wrapper.building` from PendingSubstrate to
Live.

## Scope

- Author `Milestone.emits[]` for every milestone of the Building
  chain in `src/systems/aspirations.rs`.
- Flip `aspiration_milestone_wrapper.building` to Live in
  `populate_method_registry`.

## Out of scope

- Tuning emit priorities via balance soak.
- New Building-domain DSE substrate; wraps existing Build /
  Construct primitives. Strategist-coordinator alignment is
  #335 territory, not this ticket.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #12 of 25, blocked on #319 + #321. Batch C — 5-way
parallel.

## Approach

Per htn-methods.md §H. Building milestones gate on structures
completed / structure types unlocked.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a cat with Building aspiration
  shows non-empty emit-walks.

## Log

- 2026-05-14: opened as 128 epic child #12 (Batch C chain
  follow-on).
