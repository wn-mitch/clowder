---
id: 335
title: Coordinator directives as HTN method seeds — 057 integration
status: blocked
cluster: social-coordination
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: [320]
supersedes: []
related-systems: [htn-methods.md, strategist-coordinator.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic cross-cutting integration. 057
(`coordinator-directive-intention-strategy-row`) is currently
`blocked-by: [126]`; #341 retargets to `blocked-by: [128]`. When
057 lands, its strategy row writes `HeldIntention { source:
CoordinatorDirective(coord), .. }`.

This ticket wires the **method-decomposition path** for those
directive-sourced Intentions: each `DirectiveKind` maps to a
method id; the recipient cat's L2 evaluator reads the directive,
looks up the method, adopts it with `source:
CoordinatorDirective(coord)` propagated through the root
`GoalFrame`. The recipient experiences the directive as a
multi-step arc, not an inscrutable per-tick score bump.

## Scope

- Map each `DirectiveKind` (Hunt / Forage / Build / Fight /
  Patrol / Herbcraft / Cook / SetWard / Cleanse /
  HarvestCarcass) to a method id.
- Author per-DirectiveKind methods or repurpose existing
  aspiration-domain methods where shape aligns.
- Extend the L2 evaluator (or `057`'s strategy-row landing) to
  emit `Intention::Goal { state: { label } }` from active
  directives, with the goal label matching a registered method.
- Source propagation: root `GoalFrame.source =
  IntentionSource::CoordinatorDirective(coord)` flows through
  to leaf `HeldIntention.source` per 126 + 320 plumbing.

## Out of scope

- 057's strategy-row substrate itself (that's 057's ticket).
- Trust-weighted directive momentum (ticket 130).
- Strategist-coordinator implementation (still a design note per
  `docs/systems/strategist-coordinator.md`).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #17 of 25, blocked on #320 + (external) 057 landing.

## Approach

Per htn-methods.md §Strategist-coordinator alignment. The
directive-decomposition methods are likely
`ApplicableWhen::PendingSubstrate { blocker: "057-..." }` at
#322 registration; this ticket flips them to Live alongside 057
landing. Frontmatter may grow `wires-method` once the method
catalogue stabilizes.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <recipient>` on a cat receiving a
  Coordinate directive shows method frame with `source:
  CoordinatorDirective(coord)` propagated.

## Log

- 2026-05-14: opened as 128 epic child #17 (Batch E cross-cutting).
