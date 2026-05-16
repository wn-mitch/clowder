---
id: 382
title: Influence-map based colony-district placement — retire find_building_placement spiral, plan expansion zones
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-16
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`find_building_placement` (`src/systems/coordination.rs:1267-1292`) is the
coordinator's site-spawning resolver. After a Build directive fires, it does
a spiral search outward from `colony_center` with a hardcoded radius cap of
**16 Manhattan tiles**, returning the first `footprint_valid` anchor. If
the spiral exhausts without finding a placement, the directive sits in the
queue forever waiting for "next tick" (line 1213-1216).

Diagnostic from 190's investigation (2026-05-16, seed-42 `tuned-42-095-phase-1a-shadow`):

- 6 Build directives issued across the 15-min soak
- Only 3 sites successfully spawned (all in the first 3,500 ticks)
- Tick 1,210,880: Mocha decides we need a new storehouse → **no "marks out the site" narration** → directive stuck
- Tick 1,211,840: Simba decides we need a new storehouse → **also stuck**
- Net: chronic-full latches reliably for 50,500 subsequent ticks, BuildDse
  becomes ineligible (no nearby ConstructionSite), structures_built plateaus
  at 3, welfare.shelter at 0.20

The spiral cap is brittle: as the colony places founder structures + the
first wave of directives near `colony_center`, the radius-16 spiral fills
up. The 1-tile-gap rule between buildings (`footprint_valid`) means each
new structure consumes its footprint + a 1-tile moat — easy to saturate
inside a 16-tile-radius Manhattan disk. After ~3-5 buildings near center,
every spiral position fails `footprint_valid` and `find_building_placement`
returns `None` silently.

This is not a tuning problem. Bumping the radius cap to 32 or 64 would
buy more soak time but doesn't fix the underlying shape: the coordinator
has no perception of *where the colony should grow*. The substrate-correct
answer is influence-map-based placement, mirroring the existing
`WardCoverageMap` / `WardIntentMap` / `FoxApproachCorridorMap` pattern (45
+ 301 precedent for placement-via-influence-maps).

## Scope

1. **New `ColonyDistrictMap` influence map** (or per-purpose maps:
   `StoresDistrictMap`, `DensDistrictMap`, `WorkshopsDistrictMap`,
   `WatchpostMap`). Each map encodes *desirability of placing a building
   of this kind on this tile*, composing positive signals (proximity to
   complementary structures, cat traffic, resource adjacency, expansion-
   frontier) with negative signals (predator scent, corruption,
   over-clustering of same-type, distance from access path).

2. **Retire `find_building_placement`'s spiral.** Replace with an argmax
   over the per-purpose influence map within a *reasonable scoring
   envelope* (capped at the natural map extent, not a fixed-radius
   spiral). When the argmax score is below a threshold, the directive
   can stay queued OR the coordinator can defer ("we have nowhere good
   to put this right now") — but the queued state should be observable.

3. **Stuck-directive visibility.** A directive that fails placement
   N times in a row should narrate ("Mocha looks for a spot for the
   new storehouse but the colony has grown too crowded — the plan sits
   in the back of her mind") and/or emit a diagnostic event so soaks
   can surface it. Today the failure is silent.

4. **Per-purpose semantic zones.** Different building kinds want
   different placement criteria:
   - **Stores**: near food production (Garden, Kitchen) and away from
     predator corridors; some clustering OK (warehouse district).
   - **Dens**: dispersed (cats want their own home), near safe terrain.
   - **Workshops**: near Stores (raw material access) and Hearth.
   - **Watchtowers / WardPosts**: on predator approach corridors.
   - **Garden / Farm**: on fertile terrain, near water.
   - **Midden**: away from everything else (refuse pile).

5. **Founder-relative expansion frontier.** "Colony center" should not
   be a fixed point. The frontier should slide outward as the colony
   grows, so spiral-cap-style saturation doesn't recur. The
   `cat_presence` / `CatScentMap` already encode dynamic colony footprint
   — the frontier can be derived from "where cats actually are" plus a
   buffer.

## Out of scope

- **Player-driven placement override.** A future `Place this here`
  player command (windowed UI) is orthogonal and not this ticket's job.
  This ticket fixes the autonomous coordinator path.
- **Tuning the influence map weights** beyond "ship at plausible defaults."
  The four-artifact hypothesize loop on each weight is a follow-on.
- **Modifying building footprints / removing the 1-tile-gap rule.** The
  gap rule is a real ergonomic constraint (cats need to walk between
  buildings); changing it is a different design decision.
- **Retiring `ColonyPriorityLift`** (the player-set priority flat lift
  on Build, surfaced in 190's investigation). That's a separate
  substrate-over-hacks ticket; this one is about the coordinator-driven
  autonomous placement.

## Current state

Discovered 2026-05-16 during ticket 190's diagnostic. 190's hypothesize
loop tested `build_chronic_full_weight = 0.5 → 0.7 → 1.0` and got
`structures_built = 5 → 5 → 5` across iterations. The placement bug is
the root cause — no amount of L2/L3 tuning matters when the coordinator
can't even spawn the construction sites for cats to engage with.

Companion sibling tickets opened from the same investigation:

- **373** — Den/Workshop food retrieval substrate (dark-food gap)
- **374** — Shelter as housing-security belief (per-cat home_den facet)
- **(unopened)** — `ColonyPriorityLift` retirement (pre-substrate hack)

## Approach

Builds on the existing influence-map infrastructure
(`src/systems/influence_map.rs`, `populate_influence_map_registry` in
`src/plugins/simulation.rs`). 17 InfluenceMap impls already registered;
this ticket adds one (or per-purpose several).

Implementation phases:

1. **Phase A — `ColonyDistrictMap` (or per-purpose maps).** New resource(s)
   under `src/resources/`; impl InfluenceMap; populate via dedicated
   system that reads from existing maps:
   - From `CatScentMap` → expansion frontier (cells where colony is
     active)
   - From existing `Structure` queries → "this tile is too close to a
     same-kind building" suppressor
   - From `FoxApproachCorridorMap` → "predator territory" suppressor
     for non-defensive structures (and enhancer for Watchtower / WardPost)
   - From `FoodLocationMap` / `GardenLocationMap` → "near food" enhancer
     for Stores / Kitchen / Workshop
   - From `TileMap` terrain → passability + terrain-class affinity
2. **Phase B — Placement argmax.** Rewrite `find_building_placement` to
   take a `BuildingKind` parameter, look up the per-purpose map (or
   compute composite from base map + per-purpose weights), and return
   the argmax position. Handle ties via existing jitter / deterministic
   tiebreak pattern (see 301 / 313 for ward-placement precedent).
3. **Phase C — Stuck-directive observability.** Add a counter +
   narration when a directive fails placement N times. Emit a
   diagnostic feature (`Feature::DirectiveStuckOnPlacement` or similar
   per ticket-031 step-contract pattern).
4. **Phase D — Verification.** Multi-cat scenario where the colony has
   ~6 buildings clustered near center, then issues a new Stores
   directive. Expected: placement returns a position on the expansion
   frontier (not blocked by hardcoded radius cap), the site spawns,
   a builder cat engages.

## Verification

- **Microexperiment (preferred):** new `district_placement_under_pressure`
  scenario — 8 cats + 6 founder buildings clustered tight, issue a Stores
  directive. Assert: placement returns Some, site spawns within N ticks,
  at a position that respects same-kind clustering preferences.
- **Soak verification:** seed-42 15-min soak post-landing — expect
  `structures_built` to lift materially (3 baseline → 6+ realistic, given
  6 directives are issued per soak today). Frame-diff per-DSE drift on
  Build within concordance band.
- **Survival hard-gates + continuity canaries hold.** No new mid-soak
  starvation or wildlife failures.
- **Directive-stuck narrative absent** under healthy colony growth (the
  whole point of the substrate fix is that placement no longer fails
  silently under normal play).
- `just check && just test` clean.

## Log

- 2026-05-16: opened from 190's diagnostic. The 16-tile spiral cap in
  `find_building_placement` was silently bottlenecking colony growth.
  Existing influence-map infrastructure (17 registered impls; 45 / 301
  precedent for placement-via-influence-maps in ward work) provides the
  substrate-correct pattern. User framing: "influence-map based to plan
  out 'colony districts' or do some semblance of expansion planning."
