---
id: 061
title: "Herb-location influence map producer scaffold (§5.6.3 row #8)"
status: done
cluster: null
landed-at: 52546f4
landed-on: 2026-04-28
---

# Herb-location influence map producer scaffold (§5.6.3 row #8)

**Landed:** 2026-04-28 | **Commits (1):** 52546f4 (producer scaffold; activation deferred)

**Why:** §5.6.3 row #8 of the AI substrate refactor calls for a sight × neutral herb-density map keyed by herb kind. Ticket 006 landed the four cheap colony-faction producer maps but punted herb because the per-kind shape is meaningfully different and `HerbcraftGather` lacked a target-taking variant — the map had no consumer. Ticket 061 lands the producer-side scaffolding plus the new target-taking DSE.

**What landed:**

1. **`HerbLocationMap` resource** (`src/resources/herb_location_map.rs`) — `[Vec<f32>; 8]` per-kind grids, 5-tile buckets matching the four ticket-006 maps, with `clear`/`get`/`total`/`stamp` API and lib tests for empty/OOB reads, per-kind isolation, total clamping, restamp determinism, and growth-stage strength monotonicity. Exhaustive `kind_index` match makes adding a `HerbKind` variant a build-time error.

2. **Producer writer function** (`update_herb_location_map` in `systems/magic.rs`) — defined and tested but **not yet scheduled** (see Surprises). Iterates `Harvestable` herbs, skips twisted, stamps per-kind discs weighted by growth stage (`Sprout=0.25 → Blossom=1.0`).

3. **`InfluenceMap` impl** (`systems/influence_map.rs`) — name `"herb_location"`, channel `Sight`, faction `Neutral`. `base_sample` returns `total()` (sum across kinds, clamped to 1.0).

4. **Sense-range knob** — `InfluenceMapConstants::herb_location_sense_range = 15.0`, matching the legacy `disposition.herb_detection_range` so a future cutover preserves threshold semantics.

5. **`herbcraft_target_dse`** (`src/ai/dses/herbcraft_target.rs`) — new target-taking DSE modeled on `caretake_target.rs` / `build_target.rs`. Three axes: spatial nearness (`Linear(-1, 1)` on Manhattan/range), patch-density (`Linear(1, 0)` on `HerbLocationMap.total`), maturity (`Linear(1, 0)` on growth-stage strength). Weights 0.40 / 0.40 / 0.20, `WeightedSum` composition, `Best` aggregation. Emits the same `Goal { state: "herbs_in_inventory" }` intention as `HerbcraftGatherDse` so the existing `gather_herb` step sequence dispatches unchanged. Registered in `populate_dse_registry` but no production caller invokes `resolve_herbcraft_target` yet — dormant, waiting for the activation pass.

6. **Resource insertion** in `setup.rs` alongside the four ticket-006 maps.

**Surprises surfaced:**

- **Schedule-sensitivity scar.** Registering `update_herb_location_map` in chain 1 — even with the marker cutover left untouched — collapsed Hunting and Foraging dispositions to zero on the canonical seed-42 soak (final tick 1334291 vs clean baseline 1318856; courtship 408 → 0; play 417 → 190; three new never-fired features). Bisect confirmed the writer registration alone is the trigger; disabling it returned the soak bit-identical to the `cef9137` baseline. Matches the `reconsider_held_intentions` precedent at `simulation.rs:425-433`: adding a system to the schedule reshuffles Bevy's topological sort enough to break unrelated cat behaviors. The marker cutover originally scoped here was reverted for the same reason. **Activation deferred** to a follow-on (likely paired with ticket 052's broader spatial-consideration sweep) so the four-artifact balance methodology can absorb the scheduling shift in one verification pass.

**Verification:** `just check` + `just test` (1526 lib tests pass including new `HerbLocationMap` and `herbcraft_target_dse` tests). `just verdict logs/tuned-42` is bit-identical to the `cef9137` clean baseline — same `fail` verdict on the same pre-existing canaries (3 never-fired features + mentoring=0/burial=0 documented as pre-existing in `cef9137`'s commit message). Hard survival gates pass (Starvation=0, ShadowFoxAmbush=4≤10).

---
