---
id: 340
title: Port Mating L3 chain onto HTN method
status: blocked
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: [smarter-cats, generational-continuity]
added: 2026-05-14
parked: null
blocked-by: [323]
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic worked-example landing. The Mating L3 chain
(`MoveTo → Socialize → GroomOther → MateWith`) is hand-coded in
`src/components/disposition.rs:1873-1919` per §7.M. This ticket
retargets the chain template into the method registry as
`mate_with_goal`, preserving behavior 1:1 while making the
chain inspectable from `just inspect` + trace surfaces.

Per
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Worked example: this is the demonstration that registry-driven
method composition is at parity with hand-coded chains.

## Scope

- Register `mate_with_goal` method in
  `populate_method_registry`:

```rust
Method {
    id: "mate_with_goal",
    goal_label: "mating_event_completed",
    applicable_when: ApplicableWhen::Live(mate_with_goal_applicable),
    sub_goals: &[
        Primitive { label: "approach_partner",
                    action: Action::Navigate,
                    target_hint: TargetHint::Partner },
        Primitive { label: "socialize_with_partner",
                    action: Action::Socialize,
                    target_hint: TargetHint::Partner },
        Primitive { label: "groom_partner",
                    action: Action::GroomOther,
                    target_hint: TargetHint::Partner },
        Primitive { label: "complete_mating",
                    action: Action::Mate,
                    target_hint: TargetHint::Partner },
    ],
    failure_strategy: MethodFailure::Abandon,
}
```

- Retire the hand-coded chain in `disposition.rs:1873-1919` once
  the method's behavior is verified at parity.
- Update §7.M references in `docs/systems/ai-substrate-refactor.md`
  to point at the method-driven implementation.

## Out of scope

- New mating mechanics (behavior preserved 1:1).
- §7.M Layer 1 ReproduceAspiration emission picker rows (that's
  a downstream aspiration-emits ticket).
- Joint-intention coexistence (already managed by 127 + 323).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #22 of 25, blocked on #323 (courtship_method's land
verifies the related infrastructure is solid before porting
Mating).

## Approach

Per htn-methods.md §Worked example — Mating L3 ported. The
chain template moves from `disposition.rs` into the method
registry. The Action resolvers themselves don't change; only
the harness that sequences them does.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just verdict logs/tuned-42 --vs <baseline>` shows no
  regression on mating canaries (matings, kittens_born) — the
  chain behavior is preserved 1:1.
- L3 trace shows `mate_with_goal` frame on mating-active cats.

## Log

- 2026-05-14: opened as 128 epic child #22 (Batch E cross-cutting;
  worked-example landing).
