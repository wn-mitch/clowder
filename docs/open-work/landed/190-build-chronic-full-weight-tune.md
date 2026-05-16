---
id: 190
title: Tune build_chronic_full_weight (179 follow-on)
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [190-build-chronic-full-weight.md]
landed-at: b56d97a8b58c8c265d27bb2e174a6314819e9c07
landed-on: 2026-05-16
---

## Why

Ticket 179 wired the `ColonyStoresChronicallyFull` marker into
`BuildDse` as a fourth `MarkerConsideration` axis and lifted
`default_build_chronic_full_weight` from `0.0` (dormant) to `0.5`
(plausibility). The 0.5 value is structurally chosen, not
empirically validated. Balance discipline per CLAUDE.md requires
a `just hypothesize` four-artifact loop on any axis that's been
introduced or lifted on a characteristic metric.

## Findings (2026-05-16) — ships as findings-only

**The weight tune doesn't matter at the current substrate composition.**

Two hypothesize iterations on seed-42 (n=1, duration=900s):

| Iter | Weight | structures_built | Verdict |
| --- | --- | --- | --- |
| baseline | 0.5 | 5 | n/a |
| iter-1 | 0.7 | 5 | wrong-direction (Δ=0.0%) |
| iter-2 | 1.0 | 5 | wrong-direction (Δ=0.0%) |

Layer-walk diagnosis traced the cause **upstream of BuildDse entirely**:

- L1 marker `ColonyStoresChronicallyFull` fires reliably (10,490 / 19,044
  trace-Simba L2 build-DSE eligibility checks, ~55%).
- L2 BuildDse scoring functions correctly. When eligible, Build scores
  ~0.48 (cap ~0.55 at weight=1.0 + chronic-full max contribution
  `0.15 × 1.0 = 0.15`).
- L3 selection: Build is **never top-scoring in 9,522 trace ticks for
  Simba (0.00%)**. pick_up dominates at 0.98 max. Even at weight=1.0
  Build can't approach pick_up.
- Upstream of L2: Build appears in only 46 trace lines for Simba —
  concentrated in two windows of 88 ticks each, both immediately
  following coordinator directives that *successfully* spawned a
  ConstructionSite. After tick 1,203,506, Build never enters Simba's
  evaluation at all.
- Root cause: `find_building_placement` (`src/systems/coordination.rs:1267-1292`)
  has a hardcoded 16-tile Manhattan spiral search cap. After founder
  buildings + 3 early constructions saturate the cap, every later
  directive fails placement silently (sits in queue forever). 6
  directives issued in baseline soak, only 3 sites spawned, all in
  the first 3,500 ticks.

So the substrate is doing exactly what it was designed to do. The
chronic-full latch fires, Build scores when eligible, cats select what
they can — but they can't select Build because no ConstructionSite ever
materializes after early game. Tuning the weight tunes a layer with no
work to do.

## Decision

- **Leave `build_chronic_full_weight` at 0.5** (no shipped change).
- **Land findings-only:** docs/balance/190-build-chronic-full-weight.md
  captures the iteration data and structural diagnosis.
- **Open follow-up tickets for each upstream issue surfaced:**
  - **382 (opened)** — Influence-map based colony-district placement.
    Retires `find_building_placement` spiral. This is the actual fix.
  - **373 (opened)** — Den/Workshop food retrieval substrate. Surfaced
    by 190's UI work (food in Dens/Workshops is currently dark food).
  - **374 (opened)** — Shelter as housing-security belief. Surfaced
    by parallel investigation of `welfare.shelter = 0.20`.
  - **(unopened)** — `ColonyPriorityLift` retirement (pre-substrate
    player-driven flat lift on Build; surfaced during the layer-walk
    but didn't open as ticket pending user direction).

## What 190 actually shipped (UI + observability work)

The investigation that led to the findings-only conclusion also
required UI / instrumentation extensions that landed alongside:

- `FoodStores` broadened with breakdown fields (`in_stores`, `in_dens`,
  `in_workshops`, `held`, `total_accessible()`). `current` semantics
  preserved as Stores-only for backward compatibility with all 9+
  backend readers.
- `sync_food_stores` extended to query Den + Workshop StoredItems +
  cat Inventory slots.
- `ResourcePanel` (I-key) reworked: replaced hand-bucketed 3-category
  rollup with enum-driven `ItemKind::category()`. Added breakdown
  line under the food bar.
- `ItemCategory` enum + `category()` method on `ItemKind` (5 categories:
  RawFood, Herb, Material, StorageUpgrade, Curiosity).

These UI changes are independent of the balance question — they
provide the observability the player needs to see what the new
substrate (179 + this investigation's findings) is doing.

## Out of scope

- The DSE wiring itself (179 landed it).
- Coordinator-side directive arbitration (already covered by
  existing `assess_build_pressure`).
- The placement bug fix itself (ticket 382).
- Den retrieval substrate (ticket 373).
- Shelter belief modeling (ticket 374).

## Verification

- Survival hard-gates pass at the unchanged weight (no behavioral
  regression — the weight is still 0.5 just like before).
- The UI changes pass `just check && just test` and visual screenshot
  verification (`/tmp/clowder_screenshot.png`, 2026-05-16).
- Balance doc `190-build-chronic-full-weight.md` records both
  hypothesize iterations + the structural reframe.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **210** (done, balance, score 0.90) — tune mentor_food_security_weight
- ✓ landed **211** (done, balance, score 0.89) — tune coordinate_food_security_weight
- ✓ landed **181** (done, balance, score 0.89) — Balance-tune Hunt/Forage colony_food_security saturation weights (176 follow-on)

<!-- linkages:end -->

## Log

- 2026-05-06: opened by 179's land-day follow-on. The 0.5
  plausibility default may over- or under-weight the chronic-
  full pull; validate empirically once post-wave (179+185+188)
  baseline lands.
- 2026-05-16: investigation expanded scope to include UI work
  (FoodStores breakdown + enum-driven ResourcePanel) per user
  direction. Hypothesize iterations 0.5 → 0.7 → 1.0 all returned
  structures_built = 5 (no change). Layer-walk traced root cause
  upstream to `find_building_placement` spiral cap (16-tile
  Manhattan radius silently failing after founder buildings
  saturate). Opened sibling tickets 382 (placement substrate),
  373 (Den retrieval), 374 (shelter belief). Decision: ship 190
  as findings-only with unchanged weight + opened follow-ups.
- 2026-05-16: Landed findings-only: weight unchanged at 0.5; root cause is upstream placement bug (382). UI work (FoodStores breakdown + enum-driven ResourcePanel) shipped in commit b56d97a8.
