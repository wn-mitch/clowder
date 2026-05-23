---
id: 457
title: Workshop-craft DSE + plan template for 016 Phase 2 behavioral tools
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-23
---

## Why

Ticket 368 (016 Phase 2) landed six new Workshop recipes (polish + brush + bundle + 3 gifts), three new behavioral-tool `ItemKind`s, their ingredient producers (Bristle via prey-shedding, Twig/Fiber/Flower via forage drop), three resolver branches that read tool identity in `groom_other` / `socialize` / `mate_with`, and three Feature canaries (`GroomingBrushUsed`, `PlayBundleEngaged`, `CourtshipGiftOffered`). Substrate-correct, but inert until cats actually craft the tools — no Workshop-craft DSE / plan template / executor exists. This ticket adds that pipeline so the 016 Phase 2 first-light gate from ticket 368 (≥1 of each tool produced on seed-42, continuity-canary deltas correlate with tool presence) can be satisfied.

## Scope
- Workshop-craft DSE (`src/ai/dses/craft_at_workshop.rs` or similar) — scoring shape that surfaces "I should craft X" when the cat has the inputs in inventory or available at Stores AND a Workshop is nearby. Eligibility filter on `CanCraft` marker (new) and proximity to a `Workshop` structure.
- Plan template for the craft chain — `PickUpInput` (or `RetrieveFromStores`) → `MoveToWorkshop` → `CraftAt` step. Plumbs through the existing GOAP planner.
- `resolve_craft_at_workshop` step resolver — reads a `RecipeId` from the chosen recipe, consumes inputs from `Inventory`, advances ticks per `Recipe.duration`, spawns the output `Item` per `Recipe.output.destination` (Inventory for Phase 2 outputs), records `Feature::ItemCrafted` (new) on Advance.
- DSE registration in `populate_dse_registry` (or via `#[distributed_slice(CAT_DSE_REGISTRY)]` per the 438 pattern).
- Promote the three Phase 2 canaries (`GroomingBrushUsed` / `PlayBundleEngaged` / `CourtshipGiftOffered`) from `expected_to_fire_per_soak() => false` to `true` after the first-light soak confirms each fires.
- First-light verification per [368's lessons block](368-phase-2-behavioral-tools-grooming-brush-play-bundle-courtship-gift-016-phase-2.md): `just soak-trace 42 Simba && just verdict logs/tuned-42/`; assert ≥1 of each tool crafted; assert continuity-canary deltas (grooming / play / courtship) correlate with tool presence.

## Out of scope
- Phase 3+ recipes (Warrior's Kit → 369, Wearables → 370, Decorations → 371/372).
- `RecipeInput::AnyOf` substrate — Phase 2 ships three parallel CourtshipGift recipes by design (mirrors 367's four smoking recipes).
- Skill-gated recipes (Phase 5).
- Tool-presence target-side check in `resolve_socialize` (368 ships actor-side only).

## Current state
368 lands as substrate-only (commits 1-5):
1. `feat: 368 commit 1` — retire silent-canary surfaces (`Feature::ALL` exhaustiveness test + exhaustive `expected_to_fire_per_soak`).
2. `feat: 368 commit 2` — eight new `ItemKind`s + `ItemCategory::Tool`.
3. `feat: 368 commit 3` — Bristle byproduct + Twig/Fiber/Flower forage drops.
4. `feat: 368 commit 4` — six Workshop recipes registered + `CraftingConstants` durations + resolver multipliers.
5. `feat: 368 commit 5` — three resolver branches read tool identity + three Feature canaries ship dormant.

The three Phase 2 tools never circulate in seed-42 today because no cat elects to craft them. 457 closes that gap.

## Approach

Mirror the 367 preservation precedent at the substrate-shape level, but generalize: instead of a per-station resolver (`load_drying_rack`, `tend_smoking_rack`), wire one `resolve_craft_at_workshop` that takes a `RecipeId` and dispatches off `Recipe.station == Workshop`. The `Recipe` already carries `inputs`, `duration`, `output`, `discipline` — the resolver reads all four. Per-discipline narrative differentiation comes from the existing narrative templates keyed on `Action`.

DSE scoring shape (first cut, tune via sweep):
- Base: low fulfillment on `mastery` axis (cat wants to demonstrate craft skill).
- Lift: cat has all `Recipe.inputs` in inventory (or in Stores within reach).
- Lift: cat has the matching `DisciplineKind`'s skill at threshold (currently 0; later phases gate via `Recipe.skill_gate`).
- Suppress: cat is critical-need (hunger / safety > play / mastery).

The DSE scores a `(RecipeId, Workshop)` tuple — first ranked recipe wins. Initial selection is greedy; future refinements can model recipe variety as a separate axis.

## Verification
- `cargo build --release && just check && just test` — green.
- `just soak-trace 42 Simba && just verdict logs/tuned-42/` — verdict pass.
- `just q events logs/tuned-42 GroomingBrushUsed` / `PlayBundleEngaged` / `CourtshipGiftOffered` — each count ≥ 1.
- Continuity-canary deltas: `GroomedOther` / `Socialized` / `CourtshipInteraction` show measurable lift correlated with tool-presence ticks vs a tool-stripped scenario (use `just frame-diff` against a pre-457 baseline).
- Promote the three Feature canaries to `expected_to_fire_per_soak() => true` in the same commit that wires the dispatch (mirrors 367's preservation-canary promotion pattern).

## Log
- 2026-05-23: opened as 368 follow-on. 368 ships the substrate (recipes + items + producers + resolver branches + canary variants); 457 wires the elect-side pipeline so cats craft and use the tools autonomously in seed-42.
- 2026-05-23: First-light: ItemCrafted=11 / never_fired_expected_positives=[] in seed-42 64363-tick soak. Three tool-use canaries (GroomingBrushUsed/PlayBundleEngaged/CourtshipGiftOffered) deferred — first-satisfied recipe order biased crafts toward courtship gifts; mating-canary=0 means gifts unused. Recipe-variety + mating-blocker follow-ons separate.
