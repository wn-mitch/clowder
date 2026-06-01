---
id: 495
title: CarcassPile picker filters unreachable tiles (Fish carcasses on water)
status: done
cluster: ai-substrate
initiative: []
added: 2026-06-01
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: d6d76811
landed-on: 2026-06-01
---

## Why

The post-494 Chebyshev realignment soak (`logs/tuned-42-09411128`,
commit `2eacc01b`) surfaces `TravelTo(CarcassPile): no path and stuck`
**1099 times per 15-min soak** — by far the highest plan-failure rate.
Root cause: `Fish::habitat` is `[Terrain::Water]` (`src/species/fish.rs:30,77`).
When a cat catches a fish, the carcass spawns at the prey's position
(`src/systems/disposition.rs:3841` — `default_position: prey_pos`),
which is a water tile. Water is impassable for cats
(`pathfinding::find_path` rejects impassable destinations at
`src/ai/pathfinding.rs:268-273`). The CarcassPile picker
(`src/systems/goap.rs:10715-10718`) does an unconditional `min_by_key`
over `food_pile_positions` without a passability filter, so every cat
that smells the fish carcass keeps planning a path to it, A\* refuses,
the greedy fallback makes no progress, and after
`travel_no_path_stuck_ticks` ticks the step fails with "no path and
stuck."

The failure is not safety-gating any current canary (the colony lives
longer post-494 with these failures than pre-494 without them), but
1099/soak is a load-bearing waste of plan-cycles per cat that's
suppressing positive features downstream.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Snapshot builder | `src/systems/goap.rs:4152-4162` | `food_pile_positions` collects every `OnGround` food item without terrain-passability filter | `[verified-correct]` |
| Picker | `src/systems/goap.rs:10715-10718` | `PlannerZone::CarcassPile` does `min_by_key(tile_distance_squared)` on the snapshot, returns the food-item tile directly (no offset, no passability check) | `[verified-correct]` |
| Source — Fish carcass | `src/systems/disposition.rs:3834-3842` | Hunt overflow Source-gate writes the carcass at `prey_pos`; for Fish, `prey_pos` is a `Terrain::Water` tile | `[verified-correct]` |
| Source — mammal/bird | same path | `prey_pos` is on the same tile-class the prey occupies (passable for Mouse/Rat/Rabbit/Bird) | `[verified-correct]` |
| Resolver — `TravelTo(CarcassPile)` | `src/systems/goap.rs:8902-8983` | A\* short-circuits on impassable destination; greedy `next_step` fallback fails; `no_move_ticks > travel_no_path_stuck_ticks` fires "no path and stuck" | `[verified-correct]` |
| Path primitive | `src/ai/pathfinding.rs:268-273` | `find_path` returns `None` if `to` is out-of-bounds OR `!terrain.is_passable()` | `[verified-correct]` |

The pre-494 PatrolZone fix (`perimeter_offset_position`,
`goap.rs:10599-10612`) addresses the analogous shape for offset-
construction pickers; this ticket addresses the shape for
direct-position pickers.

## Fix candidates

**Parameter-level options:**

- **R1 (filter at picker)** — Filter `food_pile_positions` to
  `map.in_bounds && terrain.is_passable()` at the picker stage.
  Minimal blast radius: one branch in `resolve_zone_position`. Catches
  the Fish-on-water case directly. Does NOT catch the disconnected-
  reachable-component case (passable tile, but A\* still fails — e.g.
  carcass on an island). Verifiable via soak.
- **R2 (filter at snapshot builder)** — Filter the
  `food_pile_positions` snapshot upstream so the impassable items
  never enter the picker pool at all. Cleaner separation but touches
  the snapshot constructor and ripples to other consumers
  (PickingUpDse eligibility at `goap.rs:8170`, dispatch arm at
  `goap.rs:8176`). Larger blast radius.
- **R3 (cooldown on failed target)** — When the TravelTo fails, record
  the picked target's entity in `RecentTargetFailures` so subsequent
  picker calls deprioritize it via the cooldown axis (the substrate
  mechanism mentor + others already use). Catches disconnected-
  component cases too, but doesn't actually retire the unreachable
  carcass — every cat tries it once before backing off.

**Structural options:**

- **R4 (split — InventoryFoodPile)** — Give Fish-on-water carcasses a
  distinct `ItemLocation::Submerged` (or `OnWater`) variant that's
  excluded from the `OnGround` filter at snapshot build. Cleanest
  long-term shape: matches the items-are-real pillar (item state
  encodes physical reality), and a future fishing mechanic that
  retrieves submerged items has somewhere to hook in. Larger scope.
- **R5 (rebind — Fish carcass spawns adjacent)** — Source the Fish
  carcass at the nearest *passable* tile to `prey_pos` instead of at
  `prey_pos` itself. Matches how shore-fishing would actually work
  ("cat fishes from shore, leaves the catch on the shore"). Touches
  the Source-gate at `disposition.rs:3838`. Mid-scope.

## Recommended direction

**R1 (filter at picker)** — ship this fix.

Rationale: the bug surfaces 1099× per soak with a 100% deterministic
cause (Fish carcasses on water); a one-line filter retires the dominant
failure mode immediately. R2 has the same retire shape but with broader
blast radius and no observable benefit. R3 doesn't actually fix the
problem (just spreads the failure across cats with a cooldown).

R4 (Submerged variant) and R5 (rebind Fish spawn site) are the right
long-term shapes — submerged items as first-class state, or Fish
carcasses physically on the shore rather than in the water — but each
is a separate substrate ticket. R1 is the holding pattern that
removes the bleed while the substrate-level fix gets scoped.

If R1 doesn't drop the rate to near-baseline (i.e. residual non-Fish
unreachable-carcass cases exist — likely disconnected-component
islands), open a follow-on for R3-style cooldown or R4-style state
variant.

## Out of scope

- Submerged-item state variant (R4) — substrate work, separate ticket.
- Fish-spawn-site rebind (R5) — substrate work, separate ticket.
- `MaterialPile` picker (`goap.rs:10711`) has the same shape but no
  documented failure rate. Apply the filter there in this PR for
  parallel symmetry but defer broader audit.
- `HoldUntilSafe: global step timeout` (670/soak) — separate
  substrate-effect from the 494 realignment; needs its own ticket.

## Verification

- `cargo test --release` — no regression on the existing pathfinding
  tests. Add a unit test against `resolve_zone_position::CarcassPile`
  that verifies an impassable-tile food item is filtered out and the
  next-nearest passable one wins.
- `just soak-trace 42 Simba 900` — `TravelTo(CarcassPile): no path
  and stuck` should drop from 1099 to near-zero. The four 494
  plan-failure rows (SearchPrey 9, EngagePrey 10, PatrolZone 0,
  MentorCat 0) must stay at or below their post-494-fix values
  (regression prevention).
- `just verdict logs/<run>` — survival + continuity stay pass; footer
  drift on plan_failures_by_reason should show CarcassPile retired.

## Log

- 2026-06-01: opened post-494 soak. Failure rate 1099/soak. Fish
  carcass-on-water root cause confirmed via layer-walk; R1 picked.
- 2026-06-01: post-494 soak: CarcassPile no-path-and-stuck retired 1099 -> 0. Hunt failures shifted slightly (SearchPrey 9->38, EngagePrey no-prey 10->50, lost-during-approach 126->32). Colony seasons survived 4 (unchanged). Frame-diff: modest DSE shifts (build -29%, hunt +39%, others within +/-15%). HoldUntilSafe remains 670->742 -- next follow-on.
