---
id: 378
title: colony material-demand axis: perceivable material-need substrate (FoodStores twin for crafting materials)
status: blocked
cluster: ai-perception
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: [375, 376, 365]
supersedes: []
related-systems: [crafting.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why
375 + 376 merely make crafting materials *exist*. This ticket makes the colony *want* them — turning material flow into perceivable substrate that DSEs read, the same way `FoodStores.colony_hunger_pressure()` shapes Hunt/Forage today. Without this layer, materials exist and recipes consume them, but **no cat is pulled toward producing them**. Production becomes a side effect of food-hunt or boredom-forage; recipes appear to fire stochastically, not in response to need. With this layer, the colony develops legible material appetites that route cat behavior — *"the reeds are going dormant; cats are stockpiling"* becomes a thing the player sees emerge, not a mechanic they have to read patch notes to know exists.

This is the OSRS-interconnection move: in OSRS, mid-game herblore demand pulls cleanup-Hunter-bird-nest behavior across the player base. The Clowder analog is a colony-level demand axis routing cat behavior toward upstream material production.

## Scope
- New resource `src/resources/material_demand.rs` with `ColonyMaterialDemand` struct. Per material (Bone, Sinew, Hide, Feather, Whisker, Organ, FishScale, Tallow, Reed, Flint, Clay, Ochre, Charcoal, Shell) tracks:
  - rolling stockpile (count across Stores + dens + workshops + held)
  - rolling production rate (last N ticks)
  - open-recipe pull (sum of recipe inputs across queued / aspired crafts; reads `RecipeRegistry` from 365)
  - seasonal-scarcity multiplier (e.g. reeds dormant in winter → 2× demand from autumn onward)
- Public method `material_pressure(MaterialKind) -> f32 ∈ [0, 1]` composing the four signals. Each signal is an **orthogonal axis** (per `feedback_single_axis_perception_scalars`); composition lives at the modifier layer, not inside any individual axis.
- New message `MaterialProduced` (`#[derive(Message)]`, verb-named) emitted by 375's `engage_prey` and 376's `resolve_harvest`. `ColonyMaterialDemand` consumes to update rolling production rate.
- DSE modifier read sites:
  - `Hunt` DSE (`src/ai/dses/hunt_target.rs`): scores prey species higher when colony needs that species' byproduct profile (e.g. rabbit-hide demand → rabbits more attractive than mice).
  - `Harvest` / `ForageHarvestable` DSE (376 introduces this — extend in this ticket): scores reed-bed vs flint-outcrop by which material is under-pressure.
- Weights per read site land in `src/resources/sim_constants.rs` with doc-comments. Default-zero on first land; lift via four-artifact methodology (`just hypothesize`).
- Trace plumbing: `material_pressure(kind)` MUST emerge as a named modifier on Hunt / Harvest DSEs in L2 trace, not a silent post-L2 bonus (per `feedback_audit_l3_disposition_mapping` + `project_l2_l3_disconnect_observation`).
- New `docs/systems/material-demand.md` design doc; cross-link from `crafting.md` and `ai-substrate-refactor.md`.

## Out of scope
- `ScoutForMaterial` DSE (explicit "go find more of X" goal when stockpile + production rate both fall below threshold and known harvestables are exhausted) — viable follow-on once the demand-pressure axis is exercised in soak.
- Rare-drop integration: rare drops are per-cat narrative fate, NOT colony aggregate demand. Conflating would re-introduce the single-axis-perception anti-pattern. Kept strictly separate.
- Food materials (raw meat etc.) are in `FoodStores`, not `ColonyMaterialDemand`. Naming should parallel: `material_pressure(kind)` mirrors `colony_hunger_pressure()`.

## Current state
Blocked on:
- **375**: producers must exist to emit `MaterialProduced` events.
- **376**: forage producers must exist for the same reason.
- **365**: `RecipeRegistry` must exist for open-recipe pull computation.

This ticket lands as the **finishing move** on the input-substrate cluster (375 / 376 / 377 / 378).

## Approach
1. Land 365 / 375 / 376 first.
2. Build `ColonyMaterialDemand` resource + `MaterialProduced` message + per-tick refresh system.
3. Wire DSE modifier reads in Hunt + Harvest; ensure each read site emits to L2 trace.
4. First-light activation per `feedback_dormant_substrate_activation_soak_first`: single `just soak-trace` + `just verdict` to confirm the layer fires; reserve `just hypothesize` for tighter follow-on tuning.

**Design pillars:**
- "Richer perception, better strategy" — each material is an orthogonal demand axis. Composition at modifier layer.
- "Substrate over hacks" — no hidden post-L2 score mutation; demand pressure is substrate visible in trace.

## Verification
- `just scenario material-demand-pressure`: preset colony with empty reed stockpile + a cat with a Reed-Mat aspiration; assert Hunt DSE does NOT elevate over Harvest DSE pointing at a reed-bed (demand pull working). Inverse: empty bone stockpile + warrior's-kit aspiration → Hunt prefers rabbit over mouse when reading bone-demand.
- `just soak-trace 42 Simba` + `just verdict`:
  - L2 trace shows `material_pressure(kind)` as named modifier on Hunt / Harvest DSEs.
  - `just frame-diff` baseline-vs-treatment: Hunt/Harvest reordering correlates with stockpile state, not just food pressure.
  - No regression on hard survival gates (Starvation == 0, ShadowFoxAmbush ≤ 10).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **176** (done, ai-substrate, score 0.90 (cross-cluster)) — cats need real inventory reasoning — trash, build-more-stores, satiation-aware…
- ✓ landed **189** (done, ai-substrate, score 0.89 (cross-cluster)) — Post-178 food_available regression — layer-walk diagnosis
- ✓ landed ** 85** (done, items-crafting, score 0.89 (cross-cluster)) — "Build-pressure farming gate: disjunctive food-or-herb demand"

<!-- linkages:end -->
## Log
- 2026-05-16: opened. Plan: `~/.claude/plans/i-d-like-to-do-bright-coral.md`. The destination of the input-substrate trajectory; 375 / 376 / 377 are prerequisites.
