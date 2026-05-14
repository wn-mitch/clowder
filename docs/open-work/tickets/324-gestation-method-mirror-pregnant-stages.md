---
id: 324
title: gestation_method — mirror Pregnant stages
status: blocked
cluster: life-cycle
initiative: [smarter-cats, generational-continuity]
added: 2026-05-14
parked: null
blocked-by: [320]
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier 1 — second live method, exercising the method
framework on a non-multi-cat aspirational arc. Mirrors
`Pregnant.stage` (Early → Mid → Late) per §7.M.7. The method
narrates the gestation progression as a queryable goal stack
without changing pregnancy.rs's authority over stage transitions.

This is the simplest possible Tier-1 method shape: three primitive
sub-goals corresponding to three time-anchored stages. Doesn't
need decomposition logic, doesn't need backtracking, doesn't need
new Actions — just registers a method whose sub-goals advance as
the existing `Pregnant.stage` advances.

## Scope

- Register `gestation_method` in `populate_method_registry` per
  [`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
  §Migration catalogue / Pregnant row.
- Three `SubGoal::Primitive` entries for Early / Mid / Late
  gestation; held-action reflects nesting / nutritional / resting
  bias per existing pregnancy.rs behavior.
- `applicable_when`: cat carries a `Pregnant` Component.
- Method advance keyed to `Pregnant.stage` transitions (the
  existing pregnancy.rs transition system).

## Out of scope

- Modifying `Pregnant` substrate or its DSE activation gate
  (§7.M.7.6 preserved).
- Postpartum / kitten-rearing arc (that's #333's `rear_kitten`).
- Late-pregnancy nesting behavior tuning (existing balance work,
  independent of this ticket).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #6 of 25, blocked on #319 + #320. Batch B Tier 1.

## Approach

Per htn-methods.md §Migration catalogue. The method *narrates*
the progression; pregnancy.rs still authors transitions. The L2
evaluator detects a cat is pregnant and the picker emits
`gestation_method` from the (future) Reproduce aspiration's
emits[] table — but as of #324 land, the method is registered
and live, and any L2 winner emitting the gestation goal label
will pick it up.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a pregnant cat shows the
  gestation_method frame in `method_stack`, with sub_goal_index
  matching the cat's current `Pregnant.stage`.
- `just verdict logs/tuned-42` shows no regression on
  generational-continuity canaries.

## Log

- 2026-05-14: opened as 128 epic child #6 (Batch B Tier 1).
