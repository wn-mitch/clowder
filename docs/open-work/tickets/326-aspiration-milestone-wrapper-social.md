---
id: 326
title: Social aspiration_milestone_wrapper + emits tables
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

128 epic Tier 1 — second chain wrapper, paired with #325 Hunting
to exercise both task-oriented (Hunting) and relationship-
oriented (Social) aspiration shapes in the same land batch.

Social milestones gate on bond formation, mentor selection, and
faction participation — structurally different from Hunting's
skill / action-count milestones. The `emits[]` table reflects
this: most rows reference Socialize-target / GroomOther / Mentor
methods, with relationship-target preconditions.

## Scope

- Author `Milestone.emits[]` for every milestone of the Social
  chain in `src/systems/aspirations.rs`.
- Each `Emit` row names label / applicable_when / strategy /
  priority per
  [`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
  §H.
- Methods referenced are either existing per-tick DSEs wrapped
  in trivial single-primitive methods, or Tier-2 dormant
  methods if the underlying substrate isn't ready.
- Register `aspiration_milestone_wrapper.social` as Live in
  `populate_method_registry`.

## Out of scope

- Cross-cat method composition (banned).
- New social DSE substrate (existing Socialize / GroomOther /
  Mentor DSEs are wrapped as primitives).
- Tuning emit priorities by balance soak.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #8 of 25, blocked on #319 + #321. Batch B Tier 1 — fully
parallel with #323 / #324 / #325.

## Approach

Per htn-methods.md §H. Social milestones (per current
`src/components/aspirations.rs`) include bond-tier targets and
mentor-completion checks. The `emits[]` rows expose these as
goal labels:

```rust
emits: &[
    Emit { label: "deepen_partner_bond", priority: Primary,
           applicable_when: has_friend_or_better_bond, .. },
    Emit { label: "mentor_apprentice", priority: Secondary,
           applicable_when: has_eligible_apprentice, .. },
    Emit { label: "participate_in_colony_gathering", priority: Tertiary,
           applicable_when: hearth_gathering_active, .. },
]
```

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a cat with the Social
  aspiration shows L1Aspiration trace records with non-empty
  emit-walks; Feature::MethodAdopted count > 0 for social-
  domain methods.
- `just verdict logs/tuned-42` shows no regression on social /
  bond canaries (grooming, mentoring, courtship).

## Log

- 2026-05-14: opened as 128 epic child #8 (Batch B Tier 1,
  paired with #325 for shape diversity).
