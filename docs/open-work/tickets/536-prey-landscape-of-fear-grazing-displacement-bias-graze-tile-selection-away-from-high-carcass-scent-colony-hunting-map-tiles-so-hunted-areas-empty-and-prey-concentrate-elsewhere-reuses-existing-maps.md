---
id: 536
title: Prey landscape-of-fear grazing displacement — bias graze-tile selection away from high carcass_scent / colony_hunting_map tiles so hunted areas empty and prey concentrate elsewhere (reuses existing maps)
status: ready
cluster: wildlife
initiative: [predator-prey-dynamics]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Predator *sign* alone should reshape where prey go, before any predator is
present — the "landscape of fear" that already governs breeding
(`prey_population`'s `predation_pressure` fear-breeding suppression) but does not
yet govern *movement*. Right now `PreyAiState::Grazing` picks its next tile with
no regard for where cats have been killing. The consequence of adding it: a cat
that hunts an area hard makes the prey **vacate** it — hunting grounds go quiet,
prey concentrate elsewhere, and the colony has to range wider as it depletes its
nearer grounds. Emergent spatial ecology from maps that already exist.

## Scope
- When `Grazing` (and idle graze-target selection) picks its next tile, weight
  candidate tiles *down* by the value at that tile in `carcass_scent_map` and
  `colony_hunting_map` (and/or `prey_scent_map` echoes of prior deaths).
- Tunable fear-weight constant(s) in `PreyConstants` controlling how strongly
  sign displaces grazing.
- Pure read of existing influence maps — no new persistent state on the prey.

## Out of scope
- Per-prey remembered danger (habituation) — that is a separate, heavier idea
  (persistent per-prey memory on the hot path); this ticket uses only the shared
  colony-scale maps.
- Real forage-node depletion (533) — composes with this (prey balance
  fear-avoidance against forage-attraction) but is independent; if 533 lands
  first the graze-tile pick becomes a two-term weighting (forage minus fear).
- Predator-side response to prey displacement (following the herd) — later.

## Current state
Nothing landed. `carcass_scent_map`, `colony_hunting_map`, and `prey_scent_map`
all exist as resources (`src/resources/`). Grazing tile selection lives in
`prey_ai` / `find_nearby_habitat_tile` (`src/systems/prey.rs`). This is a small,
map-reuse change with no new components or messages.

## Approach
1. In the graze-tile candidate scan, sample the fear maps at each candidate and
   apply a falloff-weighted penalty; pick among the least-feared passable habitat
   tiles rather than uniformly.
2. Keep it on the existing grazing cadence — no new per-tick pass. Reuse
   whatever spatial-sampling helper the maps already expose (mirror how
   Patrol/other consumers read influence maps; memory
   `project_l1_map_metadata_names` for the metadata-name divergence).
3. Guard against prey being penned into a corner or starving because all forage
   is near sign — clamp the penalty so fear biases, never hard-blocks (design
   pillar 3: compose at the modifier layer, don't zero the underlying signal;
   memory `learning_silent_zero_in_multiplicative_composition`).

## Verification
- `clowder-stat-trends` / focal: after a cat kills repeatedly in a region, local
  prey density falls and rises at the periphery; the effect decays as
  carcass/hunting sign decays.
- `just verdict`: survival + continuity canaries hold; prey do not starve from
  over-avoidance (the clamp holds) and `ShadowFoxAmbush <= 10` is not perturbed.
- Four-artifact balance framing for the movement-distribution shift.

## Log
- 2026-07-09: Opened from `/ideate` prey-ecology pass (idea #8). Confirmed the
  three fear maps exist and grazing currently ignores them. Composes with 533
  (forage attraction as the opposing term).
