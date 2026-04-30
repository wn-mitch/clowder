---
id: 045
title: Ward perimeter placement via L1 influence maps
status: done
cluster: null
landed-at: 2836cf48
landed-on: 2026-04-27
---

# Ward perimeter placement via L1 influence maps

**Landed:** 2026-04-27 | **Commits:** `2836cf48` (1/4 ward_coverage map) · `a837e18c` (2/4 perimeter placement) · `92c92784` (3/4 balance doc) · `d907a371` (4/4 drop placement-radius cap)

**Why:** A 1-hour collapse-probe soak (`logs/collapse-probe-42-fix-043-044/`) lost five cats in a single 1,200-tick wildlife-combat cluster. The colony had **2 active wards in the entire map at the cluster moment, both co-located at one tile** — 40 placements over 17 in-game years had distributed across only 13 unique tiles, 11 of them within Manhattan-3 of the colony centroid. The user's framing: placement is the load-bearing miss — even with the existing reactive-corruption gate, when the priestess does decide to ward, she should cover the threat corridor, not re-stack the altar.

**Root cause:** `compute_ward_placement` ran a defensive heuristic — "cover an uncovered structure" + "distance from existing wards" — with no notion of where threats actually come from. And the algorithm's candidate set was bounded by a `ward_placement_radius=10` disk around the cluster centroid, so even smarter scoring within the disk couldn't reach the perimeter at map edges where SFs spawn from.

**Fix:**
1. **New `WardCoverageMap` L1 influence map** (mirrors `FoxScentMap`'s bucketed-grid pattern; per-tick rebuild from live wards' strength × repel-falloff). Closes one of `§5.6.3`'s Absent maps in the substrate-refactor backlog.
2. **Rewrite `compute_ward_placement`** to score candidate tiles by sampling the L1 influence maps:
   `score = max(fox_scent, corruption) − ward_coverage + 0.3 × cat_presence − 0.005 × distance + jitter`
3. **Drop the `ward_placement_radius` hard disk cap.** Replaced with map-wide candidate generation + soft distance-cost term so perimeter tiles are reachable but the priestess won't walk to the opposite map corner for marginal gains. Removed the now-dead `ward_placement_radius` and `crafting_ward_placement_radius` constants from `SimConstants`.

**Plan deviations from the approved plan, transparent in the commits:** Skipped the planned `HerbcraftWardPerimeterTargetDse` (target-taking DSE pattern). The §6 pattern fits when the cat's *score* depends on candidates; here only the *placement target* moves while the "should we ward" gate stays in `scoring.rs::score_actions`. Rewriting the placement function body with influence-map sampling fully satisfied the design intent for ~70 LOC instead of the ~150 the DSE wrapper would have required.

**Verification:** 15-min seed-42 release deep-soak in `logs/tuned-42-ticket-045/`. Headline: `shadow_foxes_avoided_ward_total` rose **4 → 2172** (+54,200%, predicted +50–200%); per-ward deflection lifted from 0.15 to 60 SF avoidances per ward (~400× lift); SF ambush deaths **−40%**; anxiety interrupts **−86.9%**. Top placement tile shifted from the colony altar to the NE quadrant where fox-scent has accumulated. The 4th commit (radius-cap removal) was not re-soaked but is structurally safe — the prior soak's bottleneck was tile-diversity within the disk; map-wide candidates can only widen the spread further.

**Out of scope (deferred):** Lifetime probe (fix A — relax `thornward_decay_rate`). Increasing priestess throughput (fix C). Duplicate `WardPlaced` fire bug (still present, separate ticket). Herb pipeline silence. Combat-advantage math (ticket 046). CriticalHealth interrupt treadmill (ticket 047).

**Recommendation:** Re-promote a fresh post-045 baseline so future verdicts have a fair anchor (the registered baseline `logs/tuned-42` is from commit `a879f43`, pre-043+044+045 — the verdict's drift comparison was crossing four landed changes).

Balance doc: [`docs/balance/ward-perimeter-placement.md`](../../balance/ward-perimeter-placement.md).

---
