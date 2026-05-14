---
id: 325
title: Hunting aspiration_milestone_wrapper + emits tables
status: blocked
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: [321]
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier 1 — the first chain wrapper, exercising the L1→L2
emission picker on real `Aspirations` data. Wraps the existing
Hunting chain's milestones with `emits[]` tables: each milestone
names which Goal labels advance it, plus priority ordering and
applicable-when preconditions.

Chosen as Tier 1 because (a) Hunting is the worked-example
domain (Whiskers' stealth-cloak arc in
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Worked example), (b) it stress-tests skill-progression
milestone shapes — distinct from Social (#326) which exercises
relationship-shaped milestones.

## Scope

- Author `Milestone.emits[]` for every milestone of the Hunting
  chain in `src/systems/aspirations.rs`.
- Each `Emit` row names:
  - `label: &'static str` (matches a registered `Method.goal_label`).
  - `applicable_when: fn(&World, Entity) -> bool` (per-cat
    precondition).
  - `strategy: CommitmentStrategy` (typically `SingleMinded` for
    Goal Intentions).
  - `priority: Priority` (enum: Primary / Secondary / Tertiary).
- Methods that the emits[] reference must be either Live (in
  #320's registry) or PendingSubstrate (so the picker's
  registry-lookup correctly returns None and falls through).
- Register `aspiration_milestone_wrapper.hunting` as Live in
  `populate_method_registry`.

## Out of scope

- Other chains' emits[] tables (#326-#331 each own one chain).
- Authoring stealth-cloak methods (those are #322 + #334).
- Tuning emit priorities by balance soak (later balance-thread
  work).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #7 of 25, blocked on #319 + #321. Batch B Tier 1 — fully
parallel with #323 / #324 / #326.

## Approach

Per htn-methods.md §H Worked example. The Hunting chain's
current milestones (per
`src/components/aspirations.rs:48-59`) are gated on action
counts / skill levels / etc. The `emits[]` extension names the
levers that advance each milestone:

Milestone 2 ("hunt success rate ≥ 0.6") example:
```rust
emits: &[
    Emit { label: "hunt_high_value_prey", priority: Primary,
           applicable_when: high_value_prey_believed_in_range, .. },
    Emit { label: "stealth_gear_acquired", priority: Secondary,
           applicable_when: lacks_stealth_gear, .. },
    Emit { label: "stalking_skill_mentored", priority: Secondary,
           applicable_when: mentor_with_stalking_available, .. },
]
```

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (registry verifies all `label` references
  are either Live or PendingSubstrate methods).
- `just soak-trace 42 <focal>` on a cat with the Hunting
  aspiration shows L1Aspiration trace records with non-empty
  emit-walks; Feature::MethodAdopted count > 0 for hunting-
  domain methods.
- `just verdict logs/tuned-42` shows no regression on hunt /
  prey-related canaries.

## Log

- 2026-05-14: opened as 128 epic child #7 (Batch B Tier 1,
  chosen because Hunting is worked-example domain).
