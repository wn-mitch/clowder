---
id: 461
title: Phase 2b warrior's-kit TanningFrame BuildPressure threshold tuning (369 follow-on; election-completeness moves to 462/463)
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-24
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

369 landed the Phase 2b substrate (8 `ItemKind` variants + TanningFrame structure + BuildPressure channel + sibling `CraftAtTanningFrameDse` + 8 recipes + `EquipMaterial` / `WeaponClass` / `ArmorClass` / `NoiseClass` / `DurabilityTier` classifiers). 369's first-light soak (`just soak-trace 42 Simba 900` → seed-42 / commit 994d52ee / 2026-05-24) cleared survival on every death cause AND on every continuity canary but produced **zero kit items and zero Tanning Frames** — `colony_score.structures_built -27.3%` and `.fulfillment +29.7%` drove the `verdict: fail`.

This ticket was originally scoped to land the full election-completeness layer (retrieve plumbing + recipe-variety axis + threshold tuning) mirroring 367's Commit-8 / Commit-9 follow-on shape. On 2026-05-24, design review surfaced that both the retrieve plumbing and the recipe-variety axis as originally specced violate the "substrate over hacks" pillar's L2-trace clause — the recipe-variety axis lived inside the resolver (post-election lex-pick made smarter), and the retrieve plumbing relied on a generic `HasCraftInputsInStores` colony marker that collapses dozens of distinct material-flow truths into one disjunction that's true whenever the colony has *any* craft input. The substrate-honest shape (per §L2.10.5 + §7.M HTN methods + CLAUDE.md pillar #4) is `Intention::Goal(HaveItem(<ItemKind>))` emitted by a `CraftItemAspiration` chain, decomposed by a templated HTN method that reads recipe metadata. The desire itself flows through the decomposed plan; the retrieve action is parameterized by `recipe.inputs` derived from the held Intention. That refactor is too large for 461 and lands as 462 (substrate widening) + 463 (aspiration chain).

461 narrows to TanningFrame BuildPressure threshold tuning only. This still buys: cats build TanningFrames so that when 463 lands, hide gear has somewhere to craft.

## Scope

- **TanningFrame BuildPressure threshold tuning.** `build_pressure_tanning_min_hides: 5` (369's first-light default) was never met in 900 ticks. Drop to 2 (one prey-rabbit's worth) or revise the `BuildPressureTanning` signal in `update_colony_building_markers` to count hides across Stores AND inventory aggregates (so the channel fires before all hides have been deposited). First-light soak picks whichever produces ≥1 TanningFrame in 900s without over-electing.

Retrieve plumbing and recipe selection (the original §Scope bullets 1 and 2) require typed `Goal(HaveItem)` Intentions to carry recipe identity through the decomposed plan; a generic `HasCraftInputsInStores` marker collapses dozens of material-flow truths into one disjunction and is structurally wrong. Both move to sibling 462 (substrate widening) + 463 (aspiration chain).

## Out of scope

- **Retrieve plumbing** — moves to 462 (parameterized `RetrieveCraftInputs(<ItemKind set>)` whose action body reads recipe identity from the held Intention's `HaveItem` variant; no generic colony marker).
- **Recipe-variety axis / item-aspiration substrate / per-recipe L2 emission** — moves to 462 (substrate widening: `GoalKind` enum + templated HTN method) and 463 (`CraftItemAspiration` chain emits `Goal(HaveItem(recipe.output))`, retires the resolver-internal lex pick).
- **Resolver reads** for material-property effects (hunt-strike weapon-bonus, take_damage armor-reduction, ranged-attack sling-enable, movement-detection cloak-mask, noise-class detection-penalty) — separate follow-on once kit items actually exist in inventory.
- **`Feature::BoneWeaponSnapped` emitter** — lands with the resolver-reads follow-on (snap is gated on failed-strike).
- **`CraftInPlaceDse`** for open-ground knapping — 369 routed `flint_blade` through Workshop as a compromise; the no-station resolver is a separate clean-up.
- **Slot-aware equipment semantics** — depends on 017 (slot-inventory).

## Current state

369 substrate is in place and tests pass (2450 tests / `just check` green except pre-existing epic-060 drift). 461 inherits a working substrate that doesn't yet build TanningFrames because `build_pressure_tanning_min_hides: 5` is never met inside the soak window.

## Approach

1. **Drop `build_pressure_tanning_min_hides`** from 5 → 2 in `src/resources/sim_constants.rs`. One prey-rabbit's worth of hides should be enough demand to drive a TanningFrame build.
2. **If 2 still doesn't fire in the soak**, revise the `BuildPressureTanning` signal in `update_colony_building_markers` (or wherever it's authored) to count hides across both Stores aggregates AND any cat inventory, rather than just Stores. The threshold isn't the only knob — the signal-shape may be the structural fix.
3. **Re-soak** `just soak-trace 42 Simba 900` + `just verdict logs/tuned-42-<sha>/`. Iterate constant or signal-shape until ≥1 TanningFrame builds without over-electing on the rest of the soak's behavior.
4. **Open 462 and 463** via `just open-ticket` in the same landing commit (per CLAUDE.md "Antipattern migration follow-ups are non-optional").

## Verification

- `just check` + `cargo test --release --lib` pass.
- `just soak-trace 42 Simba 900` followed by `just verdict logs/tuned-42-<sha>/` — verdict pass.
- `grep -E 'TanningFrame' logs/tuned-42-<sha>/events.jsonl | grep -iE 'built|construct' | wc -l` returns ≥ 1 (a TanningFrame was constructed within the soak window).
- `structures_built` recovers part of the 27.3% deficit from 369's first-light. Full recovery (and the kit-item gate that was originally in this ticket) waits on 463.
- Continuity canaries (grooming / play / mentoring / courtship) hold or improve vs the 369 first-light baseline.

## Log

- 2026-05-24: opened as 369's election-completeness follow-on (blocked-by 369); paired ticket pattern mirrors 367 Commit-8 / Commit-9 successor relationship. Three coupled defects identified in 369's first-light soak `logs/tuned-42-994d52ee` were originally absorbed here as scope.
- 2026-05-24: narrowed to TanningFrame threshold only. Design review identified that the original scope's "retrieve-from-stores plan template" relied on a generic `HasCraftInputsInStores` colony marker that collapses dozens of distinct material-flow truths into a single disjunction (true whenever the colony has *any* craft input — zero selection information), and the "recipe-variety axis" lived inside the resolver as a post-election lex-pick refinement (leaving the L2 trace silent about which item the cat aspired to). Substrate-honest shape per §L2.10.5 + §7.M HTN methods is `Intention::Goal(HaveItem(<ItemKind>))` emitted by an aspiration chain, decomposed by a templated HTN method that reads recipe metadata — the desire itself flows through the decomposed plan steps. User framing: *"I'd have an item in mind that drives the crafting election in the first place"* + the "DesiredDryable" hint. Retrieve plumbing and recipe selection move to siblings 462 (substrate widening: `GoalKind` enum + templated HTN method + parameterized `RetrieveCraftInputs(<ItemKind set>)`) and 463 (`CraftItemAspiration` chain + retire lex pick). 461 keeps the threshold tuning because it's independent of the substrate shape and lets TanningFrames be built in the 463-era colony.
- 2026-05-24: three-iteration first-light tuning. (i) Threshold 5→2 alone (`logs/tuned-42-3a55f7af`): zero TanningFrame fires in 900s; root cause is that pre-463 cats hoard hides in inventory after `CraftAtWorkshop: no workshop recipe fully satisfied by inventory` plan failures (1350× in the run), so hides never reach Stores. (ii) Signal Stores → Stores∪Inventory at the `accumulate_build_pressure` site in `src/systems/coordination.rs` (`logs/tuned-42-042c036a`): still zero TanningFrame fires; cats hoard the supply, signal is on, but the channel's multiplier (1.0×) loses every `highest_actionable` contest against foundational channels (5.0× multipliers on `no_store` / `no_kitchen`) and the colony is in heavy infrastructure-buildout mode for the entire 900s window. (iii) Multiplier 1.0→2.0 (`logs/tuned-42-49277c72`): TanningFrame fires at tick 1214960 (Mocha, day 1215, ~15 game-days in) and completes at tick 1215044. `verdict: concern` with survival + continuity both pass; drift is the expected positive shape from intentional constants change (`colony_score.fulfillment +104.8%`, `seasons_survived +50%`, `welfare +12.3%`, `wards_placed +33.3%`). The `CraftAtWorkshop: no workshop recipe fully satisfied by inventory` plan-failure regression is the standing 369 defect that 462+463 retire, not introduced by 461. No over-election: TanningFrame fires once in the run, foundational structures all still build at expected counts (1× each of Kitchen / Workshop / Garden / DryingRack / SmokingRack, 3× Storehouse expansion which is normal storage growth).
