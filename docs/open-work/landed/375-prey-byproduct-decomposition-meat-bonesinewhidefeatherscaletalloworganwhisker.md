---
id: 375
title: prey-byproduct decomposition: meat + bone/sinew/hide/feather/scale/tallow/organ/whisker
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
landed-at: 786b727e5d5c
landed-on: 2026-05-22
---

## Why
The 016 crafting epic (`docs/systems/crafting.md`) names **bone, hide, sinew, organ, tooth, feather, fish-scale, oil/tallow** as recipe inputs for Phases 1, 2, 2b, 3, 4, and 5. Today every prey kill yields exactly one item (raw meat); none of the named byproducts have producers. `Feather` exists as an `ItemKind` but is never spawned. The crafting epic is structurally upstream-starved: phase-children 367–372 can ship recipe definitions, but the recipes will be uncraftable. This ticket plugs the producer side for guaranteed byproducts (rare-tier drops live in 377).

## Scope
- Add 7 new `ItemKind` variants in `src/components/items.rs`: `Bone`, `Sinew`, `Whisker`, `Hide`, `FishScale`, `Tallow`, `Organ` (with `decay_rate()` + `is_food() == false`).
- Extend `resolve_engage_prey` in `src/systems/goap.rs` to spawn meat + 2–3 byproducts per species:
  - Mouse → `RawMouse` + `Bone` + `Sinew`
  - Rat → `RawRat` + `Bone` + `Sinew` + `Whisker`
  - Rabbit → `RawRabbit` + `Hide` + `Bone` + `Sinew`
  - Fish → `RawFish` + `FishScale` + `Tallow` + `Organ`
  - Bird → `RawBird` + `Feather` (existing — wire producer) + `Bone`
- Verify inventory-overflow drop-to-ground path composes with multi-item spawn (already exists in `src/systems/items.rs`).
- Append a §Inputs subsection to `docs/systems/crafting.md` naming each byproduct's downstream sinks (Bone → 369/372; Sinew → 369/368; Whisker → 370/368; Hide → 369/370; FishScale → 372/371; Tallow → 371; Organ → 367; Feather → 368/370).

## Out of scope
- ShadowFox banishment byproducts → 379.
- Cat-death byproducts (heirloom-eligible bones, fur-tuft) → 380.
- Situational-trigger rare drops (LuckyRabbitsFoot etc.) → 377.
- Colony-level material demand axis that makes these byproducts felt-needed → 378.
- Metal-scrap (trader-only) → 381.

## Current state
Today: every prey kill in `resolve_engage_prey` spawns exactly one raw-meat item via `ItemSlot::new(item_kind, modifiers)`; if hunter inventory is full it drops to ground. Multi-item spawn is a clean extension of that path.

## Approach
1. Extend `ItemKind` enum + match arms for `decay_rate()`, `is_food()`, display name. Organic byproducts (Bone, Hide, Sinew, Feather, Whisker, Organ, FishScale, Tallow) decay slowly; verify against existing slow-decay items.
2. Refactor `resolve_engage_prey` to look up byproducts via a species → byproduct-list table (new `prey_byproducts` table in `src/resources/sim_constants.rs` or `src/species/`).
3. Emit one `ItemKind::*` per byproduct + the meat; reuse existing drop-to-ground overflow logic.
4. Confirm `sync_food_stores` in `src/systems/items.rs` is unaffected (new byproducts return `false` from `is_food()`).
5. Verify `Eat` DSE and resolver paths don't accidentally pull non-food byproducts (the existing `is_food()` filter is the gate).

**Design pillar:** "items are real" — these are spatial entities with real physical presence (inventory pressure, drop-to-ground, decay). No numeric modifier fields; downstream uses live on resolvers that read item identity (369 Hide-Bracers reads `Hide` presence, not a `defense_rating` float).

**Inventory pressure note:** a rabbit kill now produces 4 items instead of 1. This creates emergent pressure toward Stores deposits + cat-cooperation hauling. Surface in verdict comparison.

## Lessons from 367 first-light (inherited from [016](016-crafting-items-recipes-stations.md))

This ticket is **producer-only** (engage_prey side); the
`BuildPressure` election lesson doesn't apply (no new structure). The
ItemKind enrollment lesson and the decorative-vs-load-bearing lesson
both apply directly.

- **ItemKind enrollment audit.** Seven new variants land here (Bone /
  Sinew / Whisker / Hide / FishScale / Tallow / Organ — plus wiring
  the existing `Feather` producer). Audit every hand-maintained
  iteration constant:
  - `ItemKind` exhaustiveness test in `src/components/items.rs:~825`
    — the `[ItemKind; N]` literal needs its count bumped, otherwise
    the test silently passes on the truncated array (367 Phase 1b
    Commit 1 hit this — `[ItemKind; 33]` → `[ItemKind; 37]`).
  - `decay_rate()`, `food_value()`, `is_food()`, `category()`,
    `name()` match arms — all exhaustive, compiler catches misses.
  - Per-byproduct: confirm `is_food() == false` on the 7 new
    non-meat variants so existing `Eat` DSE / resolver paths don't
    accidentally pull `Bone` or `FishScale` as food (the 367 case for
    contrast: `is_food()` returns `true` for `DriedFish`, etc., and
    the existing eat path consumed them seamlessly).

- **No new Features expected** unless this ticket emits a
  `ByproductSpawned`-style canary. If it does (likely useful for the
  never-fired-canary): enroll the new variant(s) in
  `Feature::ALL` at `src/resources/system_activation.rs:619` (per
  367 Commit 4 amend; 5 arms total — `category()`, `feature_name()`,
  `expected_to_fire_per_soak()`, `Feature::ALL`, plus the writer at
  the producer site). Missing `Feature::ALL` is a silent
  false-negative.

- **Producer-without-consumer is a known dormancy class.** This
  ticket *creates* byproducts but no resolver *consumes* them until
  368 / 369 / 370 / 371 / 372 land. That's intentional per the
  ticket's framing — but verify in first-light that the byproducts
  actually accumulate in inventories / Stores / on the ground (i.e.
  the producer fires). The substrate-without-consumer state is
  acceptable; the substrate-without-producer state (367 first-light's
  failure mode) is not.

## Verification
- `just scenario prey-byproduct-spawn` (new scenario): preset one cat + one of each prey species at adjacent tiles; assert each kill spawns expected meat + byproducts.
- `just soak-trace 42 Simba` + `just verdict logs/tuned-42`: confirm inventory-overflow → drop-to-ground hasn't broken haul-to-Stores continuity; survival gates hold (`Starvation == 0`, `ShadowFoxAmbush ≤ 10`).
- **First-light gate (per [016](016-crafting-items-recipes-stations.md) lessons):** grep `events.jsonl` for non-zero counts of each new byproduct kind. The scenario test catches the per-species producer-table mapping; the soak catches whether seed-42 hunts actually fire in enough variety to populate every byproduct slot in a typical run.
- `just frame-diff` baseline-vs-treatment: per-DSE drift should be small; this is a producer-only change with no L2 modifier wiring.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **176** (done, ai-substrate, score 0.88 (cross-cluster)) — cats need real inventory reasoning — trash, build-more-stores, satiation-aware…
- ✓ landed **189** (done, ai-substrate, score 0.87 (cross-cluster)) — Post-178 food_available regression — layer-walk diagnosis
- ✓ landed ** 94** (done, substrate-over-override, score 0.87 (cross-cluster)) — Eat-vs-Forage IAUS imbalance — colony hauls food but doesn't consume it

<!-- linkages:end -->
## Log
- 2026-05-16: opened. Plan: `~/.claude/plans/i-d-like-to-do-bright-coral.md`. First of the four tickets in the input-substrate cluster (375 / 376 / 377 / 378) plus three follow-ons (379 / 380 / 381).
- 2026-05-19: accuracy audit pass — blocked-by empty and status ready; docs/systems/crafting.md exists; referenced ItemKind and byproduct logic are aspirational (not yet in code)
- 2026-05-22: soak logs/tuned-42-5baec8f5 (commit 5baec8f5): verdict=concern (drift vs old 095 baseline; survival+continuity gates pass). ByproductSpawned canary fired 971× and never_fired_expected_positives=[]; OverflowToGround 1027× as predicted by inventory-pressure risk note. Per-species table verified by prey_byproduct_spawn scenarios (Mouse/Rat/Rabbit/Bird) + prey_byproducts_table_default_matches_spec unit test (covers Fish row not reachable by all-Grass scenario world).
