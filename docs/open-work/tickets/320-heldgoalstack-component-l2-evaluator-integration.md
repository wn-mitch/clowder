---
id: 320
title: HeldGoalStack Component + L2 evaluator integration
status: ready
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic infrastructure. `HeldGoalStack` is the cat-side
substrate that carries the cursor through a method's sub-goal
sequence. Sibling to `HeldIntention` (126); the top frame names
*which method* and *which sub-goal index*; `HeldIntention` names
*which leaf intention*. Together they form the cat's full
actor-private commitment vector.

The L2 evaluator at `src/systems/goap.rs:568-635` is the single
authorship site for both Components. This ticket extends that
site to consult the registry when a winning DSE emits a `Goal`
Intention, push a `GoalFrame` if a method applies, and recurse
into sub-goals.

## Scope

- `src/components/held_goal_stack.rs` — `HeldGoalStack` Component
  + `GoalFrame` struct per
  [`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
  §Architecture.
- Register Component in `src/components/mod.rs`.
- Extend `IntentionSource` in `src/components/held_intention.rs`
  with `AspirationEmitted(AspirationId)` variant.
- Extend L2 evaluator at `src/systems/goap.rs:568-635` per
  htn-methods.md §L2 evaluator integration: registry lookup
  → push frame → recurse into sub-goals → adopt leaf as
  `HeldIntention`. Fall through to 126's existing direct-
  adoption path on no-method.
- Extend `resolve_goap_plans` advance/abandon logic to walk the
  stack on leaf fulfillment / abandonment per §Lifecycle.
- `MAX_DEPTH` constant (8 for Phase 1; measured-and-revised
  follow-on).
- Four new `Feature::*` variants: `MethodAdopted` (Positive,
  expected: true), `SubGoalAdvanced` (Positive, expected: true),
  `MethodBacktracked` (Neutral, expected: false),
  `MethodDepthExceeded` (Neutral, expected: false).

## Out of scope

- Authoring any specific methods (those are #323 onward).
- Trace-surface integration (#337 owns L3Commitment, #338 owns
  L1Aspiration, #339 owns CatSnapshot).
- Inspect-rendering (#336).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #2 of 25, blocked on #319. Critical-path predecessor for
Batch B (Tier 1 method landings).

## Approach

Per htn-methods.md §Architecture and §Lifecycle. The Component
classifies as substrate per §4.7.2 (no `StateEffect::Set*`
mutates it; external authorship by L2 evaluator). Serialize-only
with `Entity` targets `serde(skip)` per 126 + 127 precedent.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (substrate-stub allowlist + step contracts).
- `just soak-trace 42 <focal>` produces a trace with the new
  Features at zero count (no methods registered yet); canary
  passes vacuously.

## Log

- 2026-05-14: opened as 128 epic child #2 (Batch A infrastructure).
