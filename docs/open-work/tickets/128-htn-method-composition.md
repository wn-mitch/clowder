---
id: 128
title: HTN method composition over `HeldIntention.goal`
status: blocked
cluster: C
added: 2026-05-02
parked: null
blocked-by: [126]
supersedes: []
related-systems: [ai-substrate-refactor.md, strategist-coordinator.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Spun out of 126 (`## Out of scope`). 126 commits the goal-label
vocabulary (`ActivityKind` enum + small enumerable goal archetypes)
so that goal labels are machine-pattern-matchable. HTN
decomposition over those labels — taking a held `Intention::Goal`
and decomposing it into an ordered sequence of sub-goals via
authored *methods* — is a strategist layer *above* BDI and an
order of magnitude larger in scope than 126.

The shape this ticket has to commit:

- A `Method` registry keyed on goal label, each method carrying
  preconditions and an ordered sub-goal sequence.
- A planner that reads a cat's `HeldIntention.goal`, picks an
  applicable method, and emits the next sub-goal as the cat's
  *current* held intention while remembering the parent. Likely
  needs a `goal_stack` field on `HeldIntention` (or a sibling
  `HeldGoalStack` Component) — TBD at design.
- Method-failure → backtrack vocabulary, with Feature emissions
  that route through `record_if_witnessed` per the step-resolver
  contract.
- Read of `docs/systems/strategist-coordinator.md` to align with
  the existing coordinator design.

Not in scope here; this is the placeholder for the design.

## Dependencies

- Blocked by 126 (goal-label vocabulary must be committed; the
  `HeldIntention.goal` field is the input to method matching).
- Cluster role: C4 in 007.

## Preparation reading

- Dana Nau et al., "SHOP2: An HTN Planning System" — JAIR 2003.
- Troy Humphreys, "Exploring HTN Planners through Example" —
  *Game AI Pro* vol. 1, free online.
- `docs/systems/strategist-coordinator.md` (in repo).

## Log

- 2026-05-02: opened as 126 follow-on per CLAUDE.md
  antipattern-migration rule.
