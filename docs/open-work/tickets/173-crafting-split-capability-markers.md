---
id: 173
title: IsHerbalist / IsSpiritualist / HasCorruptionNearby capability markers (155 follow-on)
status: parked
cluster: ai-substrate
added: 2026-05-05
parked: 2026-05-05
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 155's plan called for three new L1 capability markers as
Disposition-level eligibility gates:

- **`IsHerbalist`** — gates Herbalism. Composed from
  `personality.spirituality > threshold || herbcraft_skill >
  threshold || HasHerbsInInventory || HasHerbsNearby`. Authored in
  `src/ai/capabilities.rs::update_capability_markers`. Read by the 3
  Herbcraft DSEs as eligibility require.
- **`IsSpiritualist`** — gates Witchcraft. Composed from
  `personality.spirituality > magic_affinity_threshold && magic_skill
  > magic_skill_threshold` (reusing the existing scoring thresholds
  at `src/ai/scoring.rs:1398–1399`). Read by the 6 Magic DSEs.
- **`HasCorruptionNearby`** — gates `MagicCleanse`. Promotion from
  the scalar `territory_max_corruption` to a marker authored in
  `src/systems/magic.rs` near where `OnCorruptedTile` is set. Used
  as DSE eligibility require.

The structural Crafting split landed without these (the existing
per-DSE eligibility gates — `CanCook` / `CanWard` /
`ThornbriarAvailable` / `WardStrengthLow` plus `has_herbs_nearby` /
`on_corrupted_tile` scalars — carry the substrate filter). The
soak-verdict signaled `concern` rather than `pass` partly because
the per-Disposition plan-failure cull isn't as tight as predicted
(ticket 172). Adding these capability markers would tighten the
gate at the Disposition-eligibility layer rather than the per-DSE
eligibility layer, which is the cleaner shape per CLAUDE.md
substrate-refactor §4 marker discipline.

## Plan

1. Add the three markers to `src/components/markers.rs`.
2. Author writers in `src/ai/capabilities.rs` (per-cat) and
   `src/systems/magic.rs` (per-cat / per-tile sense).
3. Wire each marker into the relevant DSE eligibility filter via
   `.require(...)` calls.
4. Rerun `just soak 42 && just verdict` and verify per-disposition
   plan-failure counts drop further (target: each below 1,000).

## Investigation hooks

The substrate-stub lint (`scripts/check_substrate_stubs.sh`) requires
each new marker to land with both reader and writer in the same
commit. The exemplar pattern is `CanWard` (per-cat capability
authored from inventory + adult-and-not-injured gate, read by
`herbcraft_ward.rs`).

## Out of scope

- Balance iteration on the threshold values used by the new markers
  — ship with the existing thresholds; tune in a follow-on if the
  soak verdict shows over-culling.

## Log

- 2026-05-05: opened by ticket 155's closeout. Markers were called
  out in the 155 plan but deferred when the structural Action+
  Disposition split alone was sufficient to land FoodCooked off the
  never-fired list. This ticket owns the L1 eligibility-gate
  tightening per CLAUDE.md substrate-refactor §4.
- 2026-05-05: parked. Ticket 172's diagnostic refactor (commit
  `055d54ee`) extended `EventKind::PlanningFailed.reason` to a typed
  `PlanningFailureReason` enum and added a per-`(disposition, reason)`
  footer aggregator. The post-172 seed-42 soak (`logs/tuned-42/`,
  commit `055d54ee`) shows **100% of the residual plan-failure
  surface — Cooking 2076 / Herbalism 1663 / Hunting 243 / Foraging 181
  — is `GoalUnreachable`**, with zero `NoApplicableActions` and zero
  `NodeBudgetExhausted`. This **rejects** this ticket's premise: the
  per-Disposition cull isn't loose because of substrate-eligibility
  gating (`NoApplicableActions` would dominate); A* finds applicable
  actions and fully explores reachable states without satisfying
  the goal predicate. Adding `IsHerbalist` / `IsSpiritualist` /
  `HasCorruptionNearby` would tighten the wrong layer. The load-
  bearing question is now: why does the L3 softmax elect Cooking /
  Herbalism for cats whose action chain — applied from the planner's
  start state — cannot reach the goal? That's an L1-marker-vs-
  PlannerState desync (the marker says "go" but the planner state
  doesn't carry the world-fact the action chain depends on) or a
  goal-predicate / action-effect mismatch. Both are different fix-
  shapes from this ticket. Parked pending a new ticket that owns
  the GoalUnreachable root-cause investigation.
