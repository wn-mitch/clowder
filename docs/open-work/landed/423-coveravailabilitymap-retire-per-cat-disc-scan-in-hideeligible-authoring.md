---
id: 423
title: CoverAvailabilityMap — retire per-cat disc scan in HideEligible authoring
status: done
cluster: combat-threat
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-19
---

## Why

Ticket 170's `update_hide_eligible_markers`
(`src/systems/sensing.rs`) authors `HideEligible` by scanning a per-cat
Chebyshev disc of `escape_viability.sprint_radius = 3` (7×7 = 49
tiles) every tick for every living cat with `HasThreatNearby`. The
code reads each tile via `map.get(x, y).terrain.is_low_cover()`.

The cost is O(cats × radius²) per tick and **scales with population**.
With kittens now surviving past the maturation gate (recent ticket
landings 397+ stabilized kitten survival), the colony grows over a
soak — and so does the disc-scan cost. Already a code smell at the
10-cat seed-42 baseline; a real problem as the colony grows toward
the spec's 20-cat steady state.

Cover availability is a **terrain property, not a per-cat property**
— two cats at the same tile have identical cover availability. The
disc scan is recomputing colony-wide-identical information per cat,
which is exactly the shape an influence map retires (precedents:
`CarcassScentMap`, `ExplorationMap`, the ticket-101 environmental
quality family).

## Scope

- **`CoverAvailabilityMap` resource** in `src/resources/cover_availability_map.rs`
  (or fold into ticket 101's environmental-quality family if 101 is
  still active when this lands). Tile-resolution `Vec<f32>` following
  the `CarcassScentMap` struct shape (`marks`, `width`, `height`,
  `get(x, y)`, `clear()`). At each cell, store the **distance to
  nearest low-cover tile** OR a normalized `[0, 1]` "has-low-cover-
  within-sprint-radius" boolean stamped outward from each
  `Terrain::is_low_cover()` tile.
- **`update_cover_availability_map` system** that rebuilds the map
  on a cadence (terrain rarely changes — buildings constructed,
  weather, fire would invalidate). Cadence controlled by an
  `EnvironmentalQualityConstants::cover_availability_update_interval`
  knob or sibling.
- **`update_hide_eligible_markers` refactor**: replace the inner
  `has_low_cover_within(pos, radius, map)` disc scan with
  `cover_map.get(pos.x, pos.y) > threshold`. O(1) per cat.
- **`impl InfluenceMap for CoverAvailabilityMap`** + registration in
  `populate_influence_map_registry` (`src/plugins/simulation.rs`)
  per CLAUDE.md's "InfluenceMap registry stubs are forbidden"
  contract.
- Preserve `Terrain::is_low_cover()` derivation — the predicate stays
  in `src/resources/map.rs`; only the per-cat scan retires.

## Out of scope

- Tuning the Hide DSE's lift constants — owned by the
  170+142+268 balance follow-on. This ticket is a substrate
  performance refactor that preserves Hide's substrate semantics
  exactly.
- Sprint-radius re-tuning — this ticket preserves the existing
  `escape_viability.sprint_radius` semantics as the map's stamp
  radius.
- Cover-quality gradient (distance-weighted) for Hide DSE scoring
  — that's a substrate-shape decision for the balance follow-on; this
  ticket can ship the map as a boolean predicate or a 0/1 cell value
  for minimal scope.

## Current state

- 170 (HideEligible authoring) landed 2026-05-19 at
  [31029c03](../landed/170-hide-eligible-authoring-system.md). The
  per-cat disc scan ships there as the v1 implementation.
- 142 and 268 landed alongside (the Hide-activation trio).
- Ticket 101 (environmental-quality influence maps) is `ready`,
  `blocked-by 100`. If 100 lands first, prefer landing this ticket
  on top of 101's `InfluenceMap<T>` shared infrastructure rather
  than reinventing.

## Approach

1. Walk `src/resources/carcass_scent_map.rs` to confirm the canonical
   shape (`marks: Vec<f32>`, `width`, `height`, `get(x, y) -> f32`,
   `clear()`, `stamp(cx, cy, peak, radius)`).
2. Implement `CoverAvailabilityMap` mirroring that shape. Stamping
   strategy options (decide at implementation time):
   - **Boolean cells:** `1.0` if any `is_low_cover()` tile is within
     `sprint_radius`, else `0.0`. Stamp from each low-cover tile
     outward.
   - **Distance cells:** `1.0 - (dist / sprint_radius)` clamped, so
     gradient information is preserved for future cover-quality
     consideration axes.
   The distance variant has more substrate texture; the boolean is a
   strict semantic-preserving refactor. Recommend boolean for v1.
3. `update_cover_availability_map` rebuilds the map on cadence. The
   stamp loop is O(low_cover_tiles × radius²) — colony-wide-amortized
   cost, paid once per cadence interval rather than per-cat-per-tick.
4. Retire the per-cat disc-scan helper `has_low_cover_within` in
   `src/systems/sensing.rs:update_hide_eligible_markers`. The
   system's signature simplifies — no more `Res<TileMap>` needed,
   only `Res<CoverAvailabilityMap>` + `Res<SimConstants>` (for
   threshold). The `Has<HideEligible>` toggle code stays.
5. Register the map's `impl InfluenceMap` and the update system
   in `SimulationPlugin::build()`. The update system runs in Chain
   2a before `update_target_existence_markers` /
   `update_hide_eligible_markers` (so reads see fresh data).

## Verification

- `just check` passes (substrate-stub + InfluenceMap registry lints
  enforce wiring).
- Unit test: the map's `get` returns `1.0` at a cell adjacent to a
  LightForest tile and `0.0` at a cell far from any low-cover terrain.
- **`just soak-trace 42 Simba` + `just verdict`** — `actions.Hide.fraction`
  and `HideEligible` toggle behavior must match 170's post-landing
  baseline (semantic preservation). If they differ, the refactor
  changed semantics — investigate.
- Performance sanity: the per-tick cost of Hide eligibility authoring
  should be O(cats) instead of O(cats × 49). With a 20-cat colony,
  the refactor should remove ~1000 tile reads per tick from the
  hot path. Measure via wall-clock tick rate vs the 170 baseline.

## Log

- 2026-05-19: opened as follow-on to 170 per user
  observation that "the disc thing is a code smell that will get
  worse as cats get more numerous (which is happening now that
  kittens survive)." The 170 trio's verification soak showed a
  41.7% wall-time tick-rate drop vs the 2026-05-15 baseline, but
  the regression is confounded by 7+ intervening commits; this
  ticket addresses the structural component (per-cat O(radius²)
  scan) independent of that diagnostic.
- 2026-05-19: Landed via just land + 15-min soak-trace verdict; pre-423 baseline archived at logs/tuned-42-pre-423/. Frame-diff shows no unacknowledged drift on tracked DSEs.
