---
id: 461
title: Phase 2b warrior's-kit election-completeness: recipe-variety axis + retrieve-from-stores plan template + TanningFrame BuildPressure tuning (369 follow-on)
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

369 landed the Phase 2b substrate (8 `ItemKind` variants + TanningFrame structure + BuildPressure channel + sibling `CraftAtTanningFrameDse` + 8 recipes + `EquipMaterial` / `WeaponClass` / `ArmorClass` / `NoiseClass` / `DurabilityTier` classifiers). 369's first-light soak (`just soak-trace 42 Simba 900` → seed-42 / commit 994d52ee / 2026-05-24) cleared survival on every death cause AND on every continuity canary but produced **zero kit items and zero Tanning Frames** — `colony_score.structures_built -27.3%` and `.fulfillment +29.7%` drove the `verdict: fail`. This ticket lands the election-completeness layer, mirroring 367's Commit-8 / Commit-9 follow-on shape (the parent epic 016 §"Lessons from 367 first-light" documents this split as the canonical Phase-N substrate-completeness → election-completeness arc).

## Scope

Three coupled fixes, all required for kit items to actually fire:

- **`[RetrieveCraftInputs, CraftAtWorkshop]` / `[…, CraftAtTanningFrame]` plan templates** mirroring 367's `[RetrieveDryable, DryFood]` shape. Prey-byproducts (Bone / Sinew / Whisker / Hide) currently deposit to Stores before cats reach a Workshop, so the `HasCraftInputInInventory` window is too narrow. The retrieve step pulls the recipe's input set from Stores back into the cat's inventory before the craft step fires. New `GoapActionKind::RetrieveCraftInputs`, sibling colony marker `HasCraftInputsInStores` (analog of `HasDryableInStores`), per-cat composite `HasCraftableAccessible` (analog of `HasDryableAccessible`).
- **Per-recipe scoring axis inside `resolve_craft_at_workshop` / `resolve_craft_at_tanning_frame`** — the §L2.10.10 "recipe variety axis" comment in `src/ai/dses/craft_at_workshop.rs:8-9` calling out the deferred refinement. Replace lex-order-first-satisfied with per-cat per-context scoring: `recency_of_threat_cue` belief facet (369's planned belief-driven impulse) for warrior's-kit recipes; role/skill match for behavioral tools; recent-use anti-monotony so cats don't loop on the same recipe. Read site for threat belief is `src/components/beliefs.rs:149`, already fed by `Attack` / `FleeFrom` / `AmbientShock` via `belief_integrator.rs:185-473`.
- **TanningFrame BuildPressure threshold tuning** — `build_pressure_tanning_min_hides: 5` (369's first-light default) was never met in 900 ticks. Drop to 2 (one prey-rabbit's worth) or revise the signal to count hides anywhere (Stores OR inventory) so the channel fires before all hides are deposited.

## Out of scope

- **Resolver reads** for material-property effects (hunt-strike weapon-bonus, take_damage armor-reduction, ranged-attack sling-enable, movement-detection cloak-mask, noise-class detection-penalty) — that's a separate follow-on. This ticket is purely about *producing* the kit items; *using* them is a sibling.
- **`Feature::BoneWeaponSnapped` emitter** — lands with the resolver-reads follow-on (snap is gated on failed-strike, which is a hunt-strike resolver concern).
- **`CraftInPlaceDse`** for open-ground knapping — 369 routed `flint_blade` through Workshop as a compromise; the no-station resolver is a separate clean-up.
- **Slot-aware equipment semantics** — depends on 017 (slot-inventory).

## Current state

369 substrate is in place and tests pass (2450 tests / `just check` green except pre-existing epic-060 drift). 461 inherits a working substrate that doesn't produce kit items; the gating analysis lives at 369's `## Log` 2026-05-24 entry. Reading order before starting: (a) `src/ai/dses/craft_at_workshop.rs` §"Scoring" rustdoc; (b) `src/steps/disposition/load_drying_rack.rs` + `src/ai/planner/actions.rs::drying_food_actions` for the 367 retrieve-chain template that 461 mirrors; (c) `docs/systems/ai-substrate-refactor.md` §L2.10.10 sibling-DSE pattern + §L2.10.6 softmax-over-Intentions.

## Approach

Substrate first, election second, threshold third (mirroring 367's commit ordering):

1. **Retrieve plumbing** — `GoapActionKind::RetrieveCraftInputs`, `resolve_retrieve_craft_inputs` resolver (reads recipe registry to know what to pull, walks the cat to Stores, transfers slots back to inventory, emits `Feature::ItemRetrieved`). Colony marker `HasCraftInputsInStores` writer in `update_colony_building_markers` (counts craft-input ItemKinds across all Stores aggregates). Per-cat composite `HasCraftableAccessible` (inventory OR stores has at least one full recipe input set), authored in `goap::evaluate_and_plan` alongside `HasDryableAccessible`. Both `CraftAtWorkshopDse` and `CraftAtTanningFrameDse` swap their `HasCraftInputInInventory` eligibility for `HasCraftableAccessible`. Plan templates extended to `[TravelTo(Stores), RetrieveCraftInputs, TravelTo(Workshop|TanningFrame), Craft*]`.
2. **Recipe-variety axis** — the resolver helper `resolve_craft_at_station` (lives at `src/steps/disposition/craft_at_workshop.rs:75-131`) currently calls `pick_satisfied_recipe(station)` which sorts lex and picks first. Replace with a scoring function that takes the cat's beliefs + skills + recent-craft-history + recipe metadata and returns the highest-scoring satisfied recipe. New `RecipeVarietyAxis` substrate per `CatRecentCrafts` Component (small ring buffer of last-N RecipeIds with tick stamps). Score components: `+threat-belief * recipe.is_warriors_kit`, `+skill-match * recipe.discipline_skill_affinity`, `-recent-use-bonus * 1/(1 + ticks_since_last_craft_of_this_id)`. Lex order falls out as the tie-break.
3. **TanningFrame threshold tuning** — lower `build_pressure_tanning_min_hides` to 2 OR count hides across both Stores AND inventory. First-light soak picks whichever produces ≥1 TanningFrame in 900s without over-electing.
4. **Re-soak `just soak-trace 42 Simba 900` + `just verdict logs/tuned-42-<sha>/`.** Gates: (a) ≥1 of each warrior's-kit ItemKind produced (or at least ≥1 across the set); (b) ≥1 TanningFrame constructed; (c) `colony_score.structures_built` recovers to within ±10% of baseline; (d) survival continues to pass.

## Verification

- `just check` + `cargo test --release --lib` pass.
- `just soak-trace 42 Simba 900` followed by `just verdict logs/tuned-42-<sha>/` — verdict pass.
- Concretely: `grep -E '(BoneStiletto|BoneTipSpear|FlintBlade|HideBracers|HidePlatedWrap|Sling|WovenReedCloak|ToothNotchedClub|TanningFrame)' logs/tuned-42-<sha>/events.jsonl | wc -l` returns > 0 (the diagnostic that returned 0 on 369's first-light).
- Continuity canaries (grooming / play / mentoring / courtship) hold or improve vs the 369 first-light baseline.

## Log

- 2026-05-24: opened as 369's election-completeness follow-on (blocked-by 369); paired ticket pattern mirrors 367 Commit-8 / Commit-9 successor relationship. Three coupled defects identified in 369's first-light soak `logs/tuned-42-994d52ee` are absorbed here as scope.
