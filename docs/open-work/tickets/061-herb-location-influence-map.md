---
id: 061
title: Herb-location influence map (§5.6.3 row #8)
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

§5.6.3 row #8 of the AI substrate-refactor spec calls for a sight ×
neutral herb-location map. Today herb sensing is a per-pair
`has_herbs_nearby` boolean with no spatial gradient — herbcraft DSEs
gate on presence/absence rather than scoring "closer is better." Spec
end-state: flat per-tile density grid, keyed by herb kind, refreshed on
growth tick and event-driven on harvest.

Ticket 006 landed the four cheap sight × colony producer maps (food,
garden, construction, kitten-urgency) but punted herb because the data
shape is genuinely different — per-tile per-kind rather than the
bucketed presence pattern the four ticket-006 maps share. Punted with
this successor open.

## Scope

1. **`HerbLocationMap` resource** at `src/resources/herb_location_map.rs`.
   Shape is *open* — per-tile flat `Vec<f32>` per herb kind (so the
   consumer can sample "thornbriar density at this tile" vs. "any-herb
   density"). Resolution decision (per-tile vs. coarser bucketed) is a
   producer-side call; the spec leaves it to implementation.
2. **Writer system** in `src/systems/magic.rs` (where herb growth lives
   today). Should run after `advance_herb_growth` so the map sees
   current growth state. Re-stamp pattern matches the substrate
   precedent.
3. **`InfluenceMap` impl** in `src/systems/influence_map.rs` with
   `metadata().name = "herb_location"`, `channel: Sight`, `faction:
   Neutral`.
4. **Sense-range / decay knobs** added to `InfluenceMapConstants` in
   `src/resources/sim_constants.rs` (the block ticket 006 introduced).
5. **Eligibility gate cutover** — the `HasHerbsNearby` marker authored
   in `sensing.rs::update_target_existence_markers` becomes a
   threshold projection of the map (`map.get(pos) > 0`) rather than a
   per-pair iteration. Same per-cat sensing scan amortizes across the
   colony via the map.

**Note on herbcraft target-taking DSE.** §5.6.3 row #8's "wanted by
DSE" column lists `HerbcraftGather` target ranking. There is *no*
target-taking DSE for herbcraft today (`herbcraft_gather.rs` is a
self-state DSE — it scores *whether* to gather, not *which* herb).
Authoring `herbcraft_target.rs` (modeled on `caretake_target.rs` /
`build_target.rs`) is **part of this ticket** — without it the map
has no consumer.

## Out of scope

- Per-DSE numeric balance tuning of the herbcraft target curve
  (lives in ticket 052 + balance threads).
- Multi-kind herb categorization beyond what `HerbcraftGather` already
  understands (Thornbriar / remedy-class / etc.) — match existing
  taxonomy.
- L2 plan-cost feedback on herb routes (ticket 052).

## Verification

- Lib tests on `HerbLocationMap` (per-tile density grid, multi-kind
  sampling, clear/restamp determinism).
- Soak verdict on canonical seed-42 deep-soak after landing — must
  return exit 0. Behavior shift expected only at the
  `HasHerbsNearby` author cutover (the map view should agree with
  the per-pair sensing predicate; if it doesn't, the cutover changed
  behavior and needs a hypothesis per balance methodology).
- Focal trace: `just soak-trace 42 Simba` — confirm
  `herb_location` sample appears in L1 records when a Simba is
  near a `Harvestable` herb.

## Log

- 2026-04-27: opened from ticket 006 closeout. §5.6.3 row #8
  promotion deferred because the per-tile-per-kind shape is
  meaningfully different from the four bucketed colony-faction maps
  ticket 006 landed, *and* because `HerbcraftGather` lacks a
  target-taking variant — herb-location wants a consumer that
  doesn't exist yet.
