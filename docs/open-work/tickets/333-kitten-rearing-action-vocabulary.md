---
id: 333
title: Kitten-rearing action vocabulary — flip rear_kitten to Live
status: blocked
cluster: life-cycle
initiative: [smarter-cats, generational-continuity]
added: 2026-05-14
parked: null
blocked-by: [320, 322]
wires-method: [rear_kitten]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 glue ticket. Authors the action vocabulary
required to flip the `rear_kitten` PendingSubstrate method to
Live: Wean / Teach / Release primitives keyed to
`KittenDependency`.

Today `KittenDependency` (in `src/components/kitten.rs`) is the
maturity-tracker on kittens; mothers care for kittens via the
per-tick Caretake DSE without a multi-tick "I am rearing kitten
X" commitment. This ticket adds the multi-stage rearing arc as a
method-decomposed aspiration on the mother.

## Scope

- New `Action::Wean`, `Action::Teach`, `Action::Release`
  variants (or refine via #322's batch; details settled during
  implementation).
- Placeholder-then-real resolvers per variant.
- Mother-side `RearKittenIntent` substrate or extension of
  existing CaretakeDse — TBD design choice.
- Flip `rear_kitten` from PendingSubstrate to Live in
  `populate_method_registry`. Author the four sub-goals: nurse
  → wean → teach → release.

## Out of scope

- Kitten-side perception of mother's intent (banned by 126
  actor-private discipline; mutually-public projection is 127
  JointIntention territory if needed).
- Tuning rearing duration / yield (balance-thread work).
- Father / partner involvement in rearing (separate aspiration
  if added later).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #15 of 25, blocked on #320 + #322. Batch D Tier 2.

## Approach

Per htn-methods.md §G Tier-2 + §Migration catalogue.
`rear_kitten(target_kitten)` keyed by the kitten Entity; one
method frame per kitten the mother is rearing. Frontmatter
`wires-method: [rear_kitten]` enforced.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (enforcement confirms wires-method
  back-reference).
- `just soak-trace 42 <mother>` on a queen with a kitten shows
  the `rear_kitten` method frame; sub-goal advances as kitten
  maturity progresses.

## Log

- 2026-05-14: opened as 128 epic child #15 (Batch D Tier 2 glue).
