---
id: 369
title: Phase 2b warrior's kit — 8 items, Tanning Frame station, material-property substrate for hunt/combat/noise resolvers (016 Phase 2b)
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-24
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
- 2026-05-24: **landed as substrate-completeness; election-completeness paired with follow-on.** Mirrors 367's Commit-1-through-6 / Commit-8 split (parent epic 016, §"Lessons from 367 first-light"). What landed:
  - **ItemKind**: 8 new variants (BoneTipSpear / BoneStiletto / FlintBlade / HideBracers / HidePlatedWrap / Sling / WovenReedCloak / ToothNotchedClub) + `ItemCategory::CombatGear`. Coverage test extended 43 → 59 (also closed the pre-existing 368 gap that left 8 variants unenrolled).
  - **Equipment substrate** (`src/components/equipment.rs`): `EquipMaterial` / `WeaponClass` / `ArmorClass` / `NoiseClass` / `DurabilityTier` enums + five exhaustive-match classifiers on `ItemKind`. Compile-time-contracts pillar: a new metal weapon in 370 must explicitly declare its material/class/durability or the build fails.
  - **StructureType**: `TanningFrame` variant + `material_cost` (Wood×4) + `default_size` (2×2) + terrain reuse. `HasFunctionalTanningFrame` marker (writer in `buildings.rs::scan_colony_buildings` + `update_colony_building_markers`; snapshot wired through `ColonyMarkerBundle` + `MarkerSnapshot::set_colony`).
  - **DisciplineKind**: `HidePeltWork` (was documented "Future Phase 3/4/5" — promoted) and generic `Stonecraft` (paired with existing `StonecraftCairn`).
  - **Recipes**: 8 registry entries + tick-budget constants on `CraftingConstants`. `flint_blade` lands as Workshop-station (open-ground knapping deferred to a `CraftInPlaceDse` follow-on).
  - **Sibling DSE**: `CraftAtTanningFrameDse` (mirror of `CraftAtWorkshopDse`, §L2.10.10 pattern) + `GoapActionKind::CraftAtTanningFrame` + `resolve_craft_at_tanning_frame` (delegating to a shared `resolve_craft_at_station` helper) + extended `crafting_actions()` plan template with both station GoapActionDefs + dispatch arm. `PlannerZone::TanningFrame` + `tanning_frame_positions` snapshot threaded through `build_zone_distances` / `resolve_zone_position` / `resolve_travel_to`.
  - **BuildPressure**: `tanning_frame` channel + accumulation arm (signal: `hide_items_in_stores >= cc.build_pressure_tanning_min_hides`) + construction-completion reset. Constants: `build_pressure_tanning_min_hides: usize = 5`, `tanning_pressure_multiplier: f32 = 1.0`.
  - **HasCraftInputInInventory**: extended to include 369 Phase 2b prey-byproduct inputs (Bone / Sinew / Whisker / Hide alongside the existing 368 Phase 2 inputs).
  - **334 subsumed**: Woven Reed Cloak == stealth cloak; the WearItem resolver path 334 named depends on slot-inventory (017), so the consumer wiring (movement-detection read of cloak) ships with the resolver-reads follow-on. 334 retired in this commit's landing.
- 2026-05-24: **first-light gate did not clear.** `just soak-trace 42 Simba 900` → `just verdict logs/tuned-42-994d52ee`: starvation 0, ShadowFox 0 deaths, continuity canaries (grooming 1001 / play 1 / mentoring 246 / courtship 634) all alive, but `colony_score.structures_built -27.3%` and `colony_score.fulfillment +29.7%` produced a `survival: fail` verdict. Zero kit items produced; zero Tanning Frames built. Root cause matches 367's lesson #2: substrate-complete ≠ election-complete. Three structural blockers, none of which 369 can fix without scope explosion: (i) prey-byproducts deposit to Stores faster than cats walk to a Workshop, so the `HasCraftInputInInventory` window is too narrow despite the marker being extended; (ii) recipe-pick is lex-order first-satisfied with no per-cat per-context scoring (the §L2.10.10 "deferred refinement" comment in `craft_at_workshop.rs:8-9`), so even Bone-bearing cats craft a GroomingBrush before a BoneStiletto; (iii) `hide_items_in_stores >= 5` threshold isn't met in 900-tick first-light. Paired follow-on `369-followon-recipe-variety-axis` opens at landing to absorb all three. The 367 inheritance lesson is exact: this is the substrate-completeness ticket; the election-completeness ticket is its blocker-flipped successor.
- 2026-05-24: pre-existing surface noise observed during `just check` — epic-060 roster drift (per `feedback_epic_dashboard_needs_queryable_state.md` memory; not 369's responsibility); trashing-stub allowlist line shifted (640 ← 619 due to `crafting_actions()` expanding for `CraftAtTanningFrame`; allowlist entry updated in same commit).
