---
id: 332
title: Grief-vigil action vocabulary — flip mourn_at_grave to Live
status: ready
cluster: life-cycle
orchestration: coherent-block
block: htn-method-composition
initiative: [smarter-cats, generational-continuity, htn-method-composition]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [mourn_at_grave]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 glue ticket. Authors the action vocabulary
required to flip the `mourn_at_grave` PendingSubstrate method to
Live: Vigil and GriefSit primitives + Grave-target picker logic.

Currently no mourning aspiration substrate exists in
`src/components/grave.rs` (Grave entities are spawned at burial
but nothing carries a "I am mourning" multi-tick state on
surviving cats). This ticket adds the action vocabulary; the
method `mourn_at_grave` (registered as PendingSubstrate in #322)
decomposes "process grief" into vigil-at-grave / grieve-in-den /
release sub-goals.

## Scope

- New `Action::Vigil` and `Action::GriefSit` variants (added via
  the substrate-stub-allowlist discipline if not already in
  #322's batch; refine during implementation).
- `resolve_vigil` / `resolve_grief_sit` step resolvers with the
  five required rustdoc headings.
- Grave-target picker logic (extends existing target-picking
  patterns from §6.3).
- Per-cat grief tracking (TBD — may need a `Mourning` Component
  carrying `deceased: Entity`, `mourning_started_tick: u64`).
  Authoritative design TBD during implementation; the substrate
  is named here for scope tracking.
- Flip `mourn_at_grave` from `ApplicableWhen::PendingSubstrate`
  to `Live` in `populate_method_registry`. Author the method's
  full sub-goal sequence (vigil → grieve-in-den → release).

## Out of scope

- §7.7.b per-relationship grief event emission (the broader
  emission-debt work tracked in 060's Phase 6b row;
  independent epic).
- Tuning grief-cascade severity (balance-thread work).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #14 of 25, blocked on #320 + #322. Batch D Tier 2 — 3-way
parallel with #333 / #334. External-substrate dependency:
mourning-aspiration may need its own Component first; if so,
this ticket grows in scope.

## Approach

Per htn-methods.md §G Tier-2 + §Migration catalogue.
`mourn_at_grave` was registered as PendingSubstrate in #322; this
ticket flips it. Frontmatter `wires-method: [mourn_at_grave]`
is the back-reference the enforcement script verifies.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (enforcement script confirms wires-method
  back-reference; `mourn_at_grave` no longer appears in
  `just methods --pending`).
- `just soak-trace 42 <focal>` on a cat near a fresh grave shows
  the `mourn_at_grave` method frame on the stack; Feature
  counts for vigil / grief-sit non-zero.

## Log

- 2026-05-14: opened as 128 epic child #14 (Batch D Tier 2 glue).
