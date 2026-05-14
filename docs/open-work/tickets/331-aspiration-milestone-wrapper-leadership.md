---
id: 331
title: Leadership aspiration_milestone_wrapper + emits tables
status: blocked
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: [321]
wires-method: [aspiration_milestone_wrapper.leadership]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 chain follow-on. Authors the Leadership chain's
`Milestone.emits[]` tables and flips
`aspiration_milestone_wrapper.leadership` from PendingSubstrate to
Live.

## Scope

- Author `Milestone.emits[]` for every milestone of the
  Leadership chain in `src/systems/aspirations.rs`.
- Flip `aspiration_milestone_wrapper.leadership` to Live in
  `populate_method_registry`.

## Out of scope

- Tuning emit priorities via balance soak.
- Coordinator-directive integration; that's #335.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #13 of 25, blocked on #319 + #321. Batch C — 5-way
parallel.

## Approach

Per htn-methods.md §H. Leadership milestones gate on coordinator-
role acceptance / directive-issued counts / mentor relationships
formed.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a cat with Leadership
  aspiration shows non-empty emit-walks.

## Log

- 2026-05-14: opened as 128 epic child #13 (Batch C chain
  follow-on).
