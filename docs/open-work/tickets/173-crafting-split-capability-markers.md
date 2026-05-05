---
id: 173
title: IsHerbalist / IsSpiritualist / HasCorruptionNearby capability markers (155 follow-on)
status: ready
cluster: ai-substrate
added: 2026-05-05
parked: null
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
