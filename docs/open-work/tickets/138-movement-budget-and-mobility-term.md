---
id: 138
title: Phase 1 — MovementBudget per entity + escape_viability mobility term
status: ready
cluster: substrate-migration
added: 2026-05-02
parked: null
blocked-by: [135]
supersedes: []
related-systems: [project-vision.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase 1 of the continuous-position migration (epic ticket 135). Lands **per-entity speed differentiation** without touching position type. Cats and wildlife still live on the integer grid; each gains a `MovementBudget` component that accumulates fractional movement points per tick at a species-specific rate. An entity gets to step a tile when its budget exceeds 1.0 (spending 1.0 per step). Sub-unit budget rates produce the right gameplay shape on the discrete grid:

- `budget_per_tick = 1.0` → every tick (today's universal default; cats, hawks, foxes, shadow foxes).
- `budget_per_tick = 0.5` → every other tick (snakes — sluggish, ambush-not-chase).
- `budget_per_tick = 1.5` → 3 moves per 2 ticks (future fast species; not used in v1).

Independent of Phase 0 (#137) and Phase 2 (#139); ships when ready. **This phase re-enables the mobility-differential term in `escape_viability`** that was punted at ticket 103's landing.

## Scope

1. **`MovementBudget` component** — `f32` accumulator + `budget_per_tick: f32` rate. Authored at spawn (cats default 1.0; wildlife species-specific from `WildSpecies` constants block).

2. **Per-tick budget accumulation.** New system in Chain 1 (before Chain 2 movement consumers): `accumulate_movement_budget` adds `budget_per_tick` to every `MovementBudget`.

3. **Movement consumers gate on budget.** Wildlife AI (`src/systems/wildlife.rs`) and any cat-side movement (the `step_toward` callers under `src/steps/`) check `budget >= 1.0` before stepping; on step success, decrement by 1.0. When budget < 1.0, the entity holds position this tick.

4. **Per-species cadence constants.** Add `default_movement_budget: f32` to the `WildSpecies` constants block in `src/resources/sim_constants.rs`. Initial values (revisit during balance pass):
   - `Hawk` = 1.0 (fast cadence; burst dive lives in a future per-ability ticket).
   - `Fox` = 1.0 (current pacing).
   - `ShadowFox` = 1.0 (faster-than-cat is a burst ability, not steady-state cadence).
   - `Snake` = 0.5 (slow — sluggish ambusher).

5. **`escape_viability` mobility term.** Extend `interoception::escape_viability` signature with `own_budget_per_tick: f32, threat_budget_per_tick: f32`. Add `mobility_weight: f32` and `mobility_normalization: f32` to `EscapeViabilityConstants`. Composition becomes:

   ```text
   mobility_advantage = clamp(
       (own_budget_per_tick - threat_budget_per_tick) / mobility_normalization,
       -1.0, 1.0
   )
   v = clamp(
       terrain_weight * openness
         + mobility_weight * (mobility_advantage * 0.5 + 0.5)
         - dependent_weight * dependent_term,
       0.0, 1.0
   )
   ```

   Cat (1.0) vs snake (0.5) → mobility_advantage = +1.0 → +mobility_weight × 1.0. Equal cadence → neutral 0.5. Slower cat → 0.0.

6. **Constants tuning.** `terrain_weight` adjusts down (e.g. 0.6) so `terrain_weight + mobility_weight + dependent_weight ≤ 1.0` keeps the scalar saturating at 1.0 for the best case.

7. **Tests.** Unit tests covering equal cadence neutral, faster cat boost, slower cat penalty, combined-with-terrain composition. Update existing `escape_viability` tests' weight assumptions.

## Verification

- `just check` / `just test` green.
- `just soak 42 && just verdict` — expect drift on Snake-related canaries.
- **Hypothesis (per CLAUDE.md balance methodology):**

  *Slowing snake cadence to 0.5 budget/tick will:*
  *(a) Reduce per-cat injury rate from snake encounters by 30–50% (cats outpace snakes).*
  *(b) Reduce snake-driven deaths to near-zero (ambushes still possible if snake catches a cornered cat, but pursuit fails).*
  *(c) Lift `escape_viability` for cats facing snakes by ~0.15–0.20 (mobility_weight × full advantage); knock-on effect on Flee selection in close-snake encounters is small because snake encounters are infrequent.*

  Run `just hypothesize` end-to-end. Promote new baseline if drift is bounded by predicted magnitude.

## Out of scope

- **Burst abilities** — hawk dive, shadow-fox lurch, etc. Each is its own ticket (own DSE, own narrative event).
- **Cat-side personality cadence** — sprightly elders, lumbering hunters. Separate axis.
- **Continuous (sub-tile) movement** — that's Phase 2 (#139). Phase 1 stays on the integer grid.

## Log

- 2026-05-02: Opened as Phase 1 of the 135 continuous-position-migration epic. Re-enables the mobility term punted at ticket 103 landing.
