---
id: 329
title: Exploration aspiration_milestone_wrapper + emits tables
status: ready
cluster: ai-substrate
orchestration: coherent-block
block: htn-method-composition
initiative: [smarter-cats, htn-method-composition]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [aspiration_milestone_wrapper.exploration]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 chain follow-on. Authors the Exploration chain's
`Milestone.emits[]` tables and flips
`aspiration_milestone_wrapper.exploration` from PendingSubstrate
to Live.

## Scope

- Author `Milestone.emits[]` for every milestone of the
  Exploration chain in `src/systems/aspirations.rs`.
- Flip `aspiration_milestone_wrapper.exploration` to Live in
  `populate_method_registry`.

## Out of scope

- Tuning emit priorities via balance soak.
- New Exploration-domain DSE substrate; wraps existing Explore /
  Wander primitives.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #11 of 25, blocked on #319 + #321. Batch C — 5-way
parallel.

## Approach

Per htn-methods.md §H. Same shape as #325. Exploration
milestones gate on tile-discovery counts / unique-region visits.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a cat with Exploration
  aspiration shows non-empty emit-walks.

## Log

- 2026-05-14: opened as 128 epic child #11 (Batch C chain
  follow-on).
