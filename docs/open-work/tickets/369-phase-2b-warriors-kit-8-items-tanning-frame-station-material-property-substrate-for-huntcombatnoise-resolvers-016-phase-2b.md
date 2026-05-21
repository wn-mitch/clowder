---
id: 369
title: Phase 2b warrior's kit — 8 items, Tanning Frame station, material-property substrate for hunt/combat/noise resolvers (016 Phase 2b)
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Land the eight Phase 2b warrior's-kit recipes — Bone-Tip Spear, Bone Stiletto, Flint Blade, Hide Bracers, Hide-Plated Wrap, Sling, Woven Reed Cloak, Tooth-Notched Club — plus the Tanning Frame station (extends Drying Rack). Items carry ecological properties (material, weapon class, noise profile, durability tier) that hunt / combat / movement / noise resolvers read, per `docs/systems/crafting.md` §Design constraints. Subsumes ticket 334 stealth-cloak as the simplest concrete consumer — 334's `blocked-by` adds 365 + 017 in the same commit that opens this ticket. Parent epic: [016](016-crafting-items-recipes-stations.md).

## Scope
- New `StructureType::TanningFrame`.
- Eight new `Recipe` entries spanning Bone & Shell Craft, Hide & Pelt Work, Stonecraft, Fiber & Weaving (per `docs/systems/crafting.md` Phase 2b table).
- Material-property substrate readable by `take_damage`, hunt-strike, ranged-attack, movement-detection, and noise resolvers (extend existing resolvers; no new resolver kinds).
- Snap-event emission for bone weapons (e.g., `BoneWeaponSnapped`) — a snapped bone-tip spear mid-hunt is a story.
- Subsume 334: stealth-cloak recipe lands here alongside the Woven Reed Cloak (or as a sibling Phase 2b sub-recipe; decide during 2b design).

## Out of scope
- Metal-bearing items (Adornment & Setting — → 370).
- Wearable slot wiring on slot-inventory (017 + → 370).

## Approach
See `docs/systems/crafting.md` Phase 2b + the material-property table (Bone / Flint / Cured hide / Fiber / Scavenged-Metal). Hypothesis: on seed-42 `--duration 900`, hunt-success rate rises ≥1.1× for equipped cats vs. unequipped; `deaths_by_cause.Starvation` remains 0; bone-weapon snap events appear in the log ≥1× per soak confirming durability mechanics fire.

## Lessons from 367 first-light (inherited from [016](016-crafting-items-recipes-stations.md))

This ticket has the **strongest inheritance** of 367's three lessons —
specifically the substrate-completeness ≠ election-completeness lesson
(367 Commit 8). When `StructureType::TanningFrame` lands, every
*mechanical* piece (construct.rs arm + state Component if needed +
recipe registry entries + Tanning DSEs + load resolver) is necessary
but **not sufficient** for the colony to ever build a Tanning Frame.
The election layer is `BuildPressure` in
`src/components/coordination.rs:144-189`; the analog 367 wiring is at
`src/systems/coordination.rs:~1110` (preservation accumulation arm) +
the `highest_actionable` channel list + the construction-completion
reset arm. Concrete checklist for this ticket:

- **(a) Add a `BuildPressure::tanning_frame` channel** alongside the
  existing `drying_rack` / `smoking_rack` channels (367 Commit 8). One
  f32 field on `BuildPressure`, one tuple in `highest_actionable`,
  one reset in the construction-completion arm.
- **(b) Decide the accumulation signal.** What colony state indicates
  "we need a Tanning Frame"? Plausible signals: `Hide` items in Stores
  ≥ threshold (the analog of 367's `raw_food_items` signal); cats with
  hunt skill ≥ threshold AND no Tanning Frame; raw hide piling up at
  Workshop unused. Pick *one* signal for first-light; iterate via
  balance ticket if elected too eagerly or too lazily.
- **(c) Add the tuning constants.** Mirror 367's
  `build_pressure_preservation_min_raw_food: usize = 5` and
  `preservation_pressure_multiplier: f32 = 1.0` in the cooking /
  storage neighborhood of `src/resources/sim_constants.rs`.
- **(d) ItemKind enrollment.** 8 new item variants land here; verify
  every hand-maintained iteration constant (`ItemKind` exhaustiveness
  test in `src/components/items.rs:~825`; any new `Feature` variants
  enrolled in `Feature::ALL` at `src/resources/system_activation.rs:619`).
- **(e) First-light soak before landing.** `just soak-trace 42 Simba`
  + `just verdict logs/tuned-42/`. Confirm: (i) at least one Tanning
  Frame is constructed in seed-42 / --duration 900; (ii) at least
  one warrior's-kit item gets produced; (iii) at least one cat
  *equips* and uses one. If any of (i-iii) fail, the substrate is
  dormant — the layer-walk doctor is at one of the three lessons in
  the epic.

This is **not** speculative scope: the 367 stack proved that without
items (a)-(e), Phase 2b ships as un-elected substrate that compiles
green but never fires.

## Verification
- `just hypothesize <spec.yaml>` runs treatment-vs-control on hunt-success with equipped/unequipped cohorts.
- `just verdict <run-dir>` — starvation canary holds, hunt rate up, snap events emitted.
- **First-light gate (per [016](016-crafting-items-recipes-stations.md) lessons):** `feature_counts.{any_warriors_kit_item_produced} >= 1` on seed-42 `--duration 900`. Until this gate clears, magnitude predictions (hunt-success lift) are unverifiable — the equipped cohort doesn't exist.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **379** (blocked, items-crafting, score 0.83) — ShadowFox banishment byproducts: shadow-bone, fox-pelt, shadow-tooth on success…
- · **377** (blocked, items-crafting, score 0.83) — rare drops & narrative items: situational-trigger rpg-expression layer (lucky r…
- · **378** (blocked, ai-perception, score 0.82 (cross-cluster)) — colony material-demand axis: perceivable material-need substrate (FoodStores tw…

<!-- linkages:end -->
## Log
- 2026-05-16: opened as 016 epic decomposition (Phase 2b; parent 016, blocked-by 365).
- 2026-05-19: accuracy audit pass — blocked-by clear (365 landed 2026-05-14); status ready verified; related-work 334/379/377/378 exist in tickets
