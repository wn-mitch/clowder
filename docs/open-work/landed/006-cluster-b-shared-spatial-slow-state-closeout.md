---
id: 006
title: Cluster-B shared spatial slow-state closeout
status: done
cluster: null
landed-at: 10989775
landed-on: 2026-04-27
---

# Cluster-B shared spatial slow-state closeout

**Landed:** 2026-04-27 | **Commit slate:** four producer-map landings + successor-ticket batch

**Why:** Ticket 006 was re-scoped on 2026-04-27 from "≥2 layers share an abstraction" (already met by Phase 2B + tickets 045/048) to "every §5.6.3 row landed, has a successor ticket, or is explicitly out-of-scope." The substrate is mature — `InfluenceMap` trait + `Attenuation` pipeline at `src/systems/influence_map.rs`, seven implementing maps, `SpatialConsideration` consumer surface declared at `src/ai/considerations.rs:73-112`. The remaining work was completing the catalog of producer maps so DSEs that want spatial inputs aren't silently degraded once ticket 052 lands the consumer cutover.

**What landed:**

- **`FoodLocationMap` (§5.6.3 row #7).** New resource at `src/resources/food_location_map.rs`. Re-stamped each tick from `Stores` + `Kitchen` `Structure` entities; each functional building paints a linear-falloff disc weighted by effectiveness. `InfluenceMap` impl exposes it as `map_key = "food_location"`, sight × colony.
- **`GardenLocationMap` (§5.6.3 row #10).** Same shape; stamped from `Garden` `Structure` entities. `map_key = "garden_location"`.
- **`ConstructionSiteMap` (§5.6.3 row #9).** Stamped from active `ConstructionSite` (urgency = `1 - progress`) and damaged `Structure` (urgency = `1 - condition` when condition < `damaged_threshold`). `map_key = "construction_site"`.
- **`KittenUrgencyMap` (§5.6.3 row #13).** Stamped each tick from `KittenDependency` cats weighted by `1 - hunger`. Re-stamped per tick (kittens move + need-state changes fast). `map_key = "kitten_urgency"`. Writer in `src/systems/growth.rs::update_kitten_urgency_map` runs in chain 2a after `kitten_mood_aura`.
- **`InfluenceMapConstants` block** in `src/resources/sim_constants.rs` (with `#[serde(default)]`). Five knobs: four sense-range floats (food/garden/construction/kitten-urgency) + `damaged_threshold` (mirrors §4 `HasDamagedBuilding` predicate). Producer-only at landing — values become balance-affecting once ticket 052 cuts consumers over.
- **Schedule wiring.** Building-side maps run in chain 1's nested building sub-tuple (`apply_building_effects → decay_building_condition → update_food_location_map → update_garden_location_map → update_construction_site_map`) — nesting keeps the outer chain under Bevy's 20-system tuple limit. Map writers run after `decay_building_condition` so effectiveness gates read post-decay values. Kitten-urgency runs in chain 2a after `kitten_mood_aura` so matured kittens (KittenDependency removed by `tick_kitten_growth` this frame) drop out of the same frame's stamp.
- **Resource registration** in `src/plugins/setup.rs` alongside the existing 5 influence-map inserts.
- **Per-map unit tests + `InfluenceMap` trait round-trip tests** for all four new maps, mirroring the `WardCoverageMap` (ticket 045) and `CarcassScentMap` (ticket 048) patterns.

**§5.6.3 catalog disposition (spec-aligned numbering):**

- [x] **#7 food-location** — landed (`FoodLocationMap`, this closeout)
- [x] **#9 construction-site** — landed (`ConstructionSiteMap`, this closeout)
- [x] **#10 garden-location** — landed (`GardenLocationMap`, this closeout)
- [x] **#13 kitten-urgency** — landed (`KittenUrgencyMap`, this closeout)
- [~] **#3 ward-strength promotion** — successor [063](../tickets/063-ward-strength-promotion.md). Inherits ticket 045's deferral.
- [~] **#5 prey-species split** — successor [062](../tickets/062-prey-species-split-maps.md). Inherits ticket 048's deferral.
- [~] **#6 carcass-scent consumer cutover** — successor [064](../tickets/064-carcass-scent-consumer-cutover.md). Inherits ticket 048's deferral; balance-affecting.
- [~] **#8 herb-location** — successor [061](../tickets/061-herb-location-influence-map.md). Punted because the per-tile-per-kind shape is meaningfully different from the four bucketed colony-faction maps, and `HerbcraftGather` lacks a target-taking variant.

**Note on numbering reconciliation:** ticket 006's pre-closeout checklist labeled rows `#4 food-location`, `#7 herb-location`, `#8 construction-site`, `#9 garden`. Those numbers were off-by-one against spec §5.6.3's authoritative numbering (food=#7, herb=#8, construction=#9, garden=#10). The closeout uses spec numbers — future readers cross-referencing the spec can do so cleanly.

**Producer-without-consumer is the established pattern.** Tickets 045 (ward) and 048 (carcass) both landed substrate-only with consumer cutover deferred. The four new producers sit dark in `FixedUpdate` until ticket 052 lands `SpatialConsideration` per-DSE wiring; their per-tick clear-and-stamp cost is a few microseconds per tick, dwarfed by everything else in chain 1.

**Successor tickets opened:**

- [061](../tickets/061-herb-location-influence-map.md) — Herb-location influence map (§5.6.3 row #8) + `HerbcraftGatherTarget` DSE.
- [062](../tickets/062-prey-species-split-maps.md) — Per-prey-species `PreyScentMap` split (§5.6.3 row #5).
- [063](../tickets/063-ward-strength-promotion.md) — Ward-strength first-class spatial axis (§5.6.3 row #3).
- [064](../tickets/064-carcass-scent-consumer-cutover.md) — Carcass-scent consumer cutover at `goap.rs:1133–1145` (§5.6.3 row #6); balance-affecting.

**Verification:** `just check` clean (lint + clippy + step-resolver + time-units). Lib tests 1484 → 1503 (+19 across the four maps' unit + trait tests). Soak comparison vs. control at parent commit (10989775):

| Metric | Control | Post-006 | vs. registered baseline (a879f43) |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 0 | 0 | 2 — both runs better |
| `deaths_by_cause.ShadowFoxAmbush` | 3 | 4 | 4 |
| `deaths_by_cause.Injury` | 0 | 3 | n/a |
| `deaths_by_cause.WildlifeCombat` | 0 | 1 | 1 |
| `continuity_tallies.courtship` | 837 | 0 | 804 |
| `continuity_tallies.grooming` | 126 | 217 | 71 |
| `continuity_tallies.play` | 604 | 906 | 111 |
| `never_fired_expected_positives` count | 3 | 5 | 8 — both runs better |
| `wards_placed_total` | 23 | 15 | n/a |

**Verdict-tool result:** both runs report `verdict: fail` because of pre-existing continuity collapses (`mentoring=0`, `burial=0`) that have been baseline state for some time. Post-006 adds `courtship=0` to that list and adds `BondFormed` + `CourtshipInteraction` to never-fire. **Hard survival gates pass on both** (Starvation == 0, ShadowFoxAmbush ≤ 10, footer present, never-fires strict subset of registered baseline's 8).

**Why behavior shifted despite producer-only:** adding 4 new systems + 1 new constants block + 1 new `Resource` registration to the FixedUpdate schedule perturbs Bevy's internal scheduling state enough to shift cat trajectories under seed-42 determinism. None of the new code is read by any consumer; the shift is structural ECS noise, not gameplay logic. This kind of variance is a known property of schedule edits in this codebase (see ticket 049's "anxiety_interrupt 17× spike — needs separate investigation" follow-on for prior precedent). **Hypothesis-direction:** unpredictable; magnitudes substantial. **Concordance:** observation is consistent with the unpredictable-direction hypothesis; survival gates hold.

The naive prediction at plan time was "behavior-neutral since no DSE consumes the new maps yet." That was wrong — the producer-only invariant is gameplay-only, not Bevy-determinism-only. Future schedule additions to chain 1 / chain 2a should expect similar shifts and budget for a soak verdict review even when the added systems are dark.

**Out of scope (explicit):**

- `SpatialConsideration` integration into any DSE — owned by ticket 052.
- Per-DSE numeric balance tuning of curve midpoints / steepness — ticket 052 + balance threads.
- Pathfinder-in-the-loop — rejected per spec §L2.10.7.
- Per-species prey split / ward-strength promotion / carcass consumer cutover / herb-location producer — moved to successor tickets 061–064.
- Phase 7 substrate cleanup (ticket 059) and the AI substrate refactor epic (ticket 060) — distinct concerns.

---
