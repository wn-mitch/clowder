---
id: 327
title: Combat aspiration_milestone_wrapper + emits tables
status: ready
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [aspiration_milestone_wrapper.combat]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 chain follow-on. Authors the Combat chain's
`Milestone.emits[]` tables and flips
`aspiration_milestone_wrapper.combat` from
`ApplicableWhen::PendingSubstrate` (registered at #321 land) to
`Live`.

Per the universal-aspiration discipline
([`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§G + CLAUDE.md "Every dormant method has a glue ticket"): this
ticket is the glue. The frontmatter `wires-method` field is
the back-reference the enforcement script verifies.

## Scope

- Author `Milestone.emits[]` for every milestone of the Combat
  chain in `src/systems/aspirations.rs`.
- Per `Emit`: `label` (registered method), `applicable_when`
  precondition, `strategy`, `priority` (Primary/Secondary/Tertiary).
- Flip `aspiration_milestone_wrapper.combat` from PendingSubstrate
  → Live in `populate_method_registry`.

## Out of scope

- Tuning emit priorities via balance soak (later balance-thread
  work).
- New Combat-domain DSE substrate; this ticket wraps existing
  Fight / Patrol / Flee primitives.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #9 of 25, blocked on #319 + #321. Batch C — 5-way parallel
with #328-#331. Can overlap Batch B.

## Approach

Per htn-methods.md §H + Tier-1/Tier-2 split. Same shape as #325
Hunting wrapper; differs only in chain-specific milestone
predicates and emit lever set.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (registry verifies wrapper is now Live;
  enforcement script confirms `wires-method` is consumed).
- `just soak-trace 42 <focal>` on a cat with Combat aspiration
  shows L1Aspiration emit-walks with non-empty rows.

## Log

- 2026-05-14: opened as 128 epic child #9 (Batch C chain
  follow-on).
