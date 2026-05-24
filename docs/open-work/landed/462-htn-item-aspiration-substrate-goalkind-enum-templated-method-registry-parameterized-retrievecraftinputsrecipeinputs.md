---
id: 462
title: HTN item-aspiration substrate: GoalKind enum + templated method registry + parameterized RetrieveCraftInputs(recipe.inputs)
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-24
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md, ai-substrate-refactor.md]
related-balance: []
landed-at: 14d83dc3
landed-on: 2026-05-24
---

## Why

461's design review (2026-05-24) identified that the substrate-honest shape for recipe selection in crafting is `Intention::Goal(HaveItem(<ItemKind>))` emitted by an aspiration chain, decomposed by a **templated** HTN method that reads recipe metadata at decomposition time. The desire itself flows through the decomposed plan; the retrieve action is parameterized by `recipe.inputs` derived from the held Intention. This avoids the two pillar violations the original 461 plan carried: (a) a generic `HasCraftInputsInStores` colony marker that collapses dozens of distinct material-flow truths into one disjunction (true whenever the colony has *any* craft input — zero selection information), and (b) a resolver-internal lex pick made "smarter" with a recipe-variety axis, which still leaves the L2 trace silent about *which* item the cat aspired to.

462 lands the substrate widening. 463 lifts the dormant weight by emitting `Goal(HaveItem)` from `CraftItemAspiration`. The split keeps the verdict surfaces small — 462 ships with zero behavior change, 463's verdict isolates the per-recipe L2 emission.

## Scope

- **Widen `GoalState`** at `src/ai/dse.rs:103-110`. Replace the flat struct with a `GoalKind` enum:
  ```rust
  pub enum GoalKind {
      Predicate { label: &'static str, achieved: fn(&World, Entity) -> bool },
      HaveItem(ItemKind),
  }
  pub struct GoalState { pub kind: GoalKind }
  ```
  `HaveItem(item)` carries a uniform achievement predicate (the cat's inventory contains `item`). `label()` derives `"have_<item>"` for trace emission. Mechanical rewrite of every `GoalState { label, achieved }` literal across the codebase — enumerate via `grep -rn "GoalState {" src/` during implementation.
- **Templated HTN method support.** `populate_method_registry` (`src/plugins/simulation.rs:823-929`) currently registers one `Method` per `goal_label: &'static str`. Add a parallel registration path that matches on `GoalKind::HaveItem(_)` and decomposes by reading the recipe registry at decomposition time: `[Primitive(RetrieveCraftInputs(recipe.inputs)), Primitive(TravelTo(recipe.station)), Primitive(Craft<station>(recipe.id))]`. The decomposition is **data-driven over `recipe.station` + `recipe.inputs`** — one templated method handles all current and future recipes.
- **Parameterized `GoapActionKind::RetrieveCraftInputs(<ItemKind set>)`** in `src/ai/planner/actions.rs`. New variant carrying the recipe's input ItemKinds as parameters. Resolver `resolve_retrieve_craft_inputs` (new file under `src/steps/disposition/`) walks the cat to Stores and pulls only the specific named ItemKinds from reachable aggregates; emits `Feature::ItemRetrieved`. Five-heading rustdoc per the Step Resolver Contract. **No generic colony marker** — the action is reachable only via the templated method, which reads recipe identity from the held `Intention::Goal(GoalKind::HaveItem(_))`.
- **Dormant.** 462 ships substrate-only. `GoalKind::HaveItem` is unreachable until 463 emits it. Existing `pick_satisfied_recipe` lex pick stays intact (no behavior delta from 461).

## Out of scope

- **Aspiration emit** (in 463 — the `CraftItemAspiration` chain that actually produces `Goal(HaveItem)` Intentions).
- **DryFood / Cook conversion to typed Goals.** 367's `HasDryableInStores` / `HasDryableInInventory` / `HasDryableAccessible` shape stays intact for now. A separate sibling against 462 can convert them after 463 proves the typed-Goal shape works.
- **Retirement of resolver-internal lex pick** in `resolve_craft_at_*` — in 463 (the lex pick becomes unreachable once 463's aspiration emits typed Goals; 463 can delete it or keep as fallback).
- **Generic colony marker analogs** of 367's `HasDryableInStores` for craft inputs — explicitly NOT introduced. This is the load-bearing distinction from the original 461 plan.

## Current state

- 461 lands TanningFrame BuildPressure threshold tuning (independent of substrate shape).
- 369's substrate (8 warrior's-kit ItemKinds, recipes, classifiers) is in place.
- `GoalState.label: &'static str` shape is at `src/ai/dse.rs:103-110`; the existing usage pattern is documented in `src/ai/dse.rs:207` (`Intention::Goal { state: GoalState, .. }`).
- `populate_method_registry` registers 8+ methods today (rear_kitten, caretake_kitten, courtship_method, hunt_method, etc.). All use `&'static str` goal labels; none decompose item-aspiration.
- No precedent exists for parameterized `GoapActionKind` variants carrying `ItemKind` collections — author's call on SmallVec / Vec / array sizing during implementation.

## Approach

Substrate-only landing, three commits:

1. **GoalState → GoalKind**. Rewrite `src/ai/dse.rs:103-110` to introduce the enum. Add `impl GoalState` helpers (`label()`, `achieved(world, entity)`) so call sites stay readable. Mechanical rewrite of every `GoalState { label, achieved }` literal — `grep` enumerates the count. `just check` + `cargo test --release --lib` green between every commit in this step.
2. **Parameterized RetrieveCraftInputs**. Add the `GoapActionKind` variant + resolver. Variant carries `SmallVec<[ItemKind; 4]>` or `Vec<ItemKind>` (author's call — most recipes carry 1-3 inputs, so SmallVec avoids heap allocation in the hot path). Resolver enumerates the cat's reachable Stores aggregates, picks the closest one carrying the named ItemKinds, walks there, transfers. No-op if no aggregate carries the inputs (return witnessless StepOutcome — the plan failed; HTN will replan or aspire elsewhere).
3. **Templated method**. Add a parallel registration path in `populate_method_registry` that intercepts `Goal { kind: HaveItem(item) }` decomposition by reading the recipe registry at decomposition time. The recipe registry's `recipe.station` + `recipe.inputs` produce the sub-goal sequence. Unit test asserts a synthetic `Goal(HaveItem(BoneTipSpear))` decomposes to `[RetrieveCraftInputs(Bone+Sinew), TravelTo(Workshop), CraftAtWorkshop(BoneTipSpearRecipe)]`, NOT a generic input set.

## Verification

- `just check` + `cargo test --release --lib` green. Compile-shape changes only.
- New unit test in `src/ai/methods/` (or wherever method tests live): synthesize `Goal(GoalKind::HaveItem(BoneTipSpear))`, decompose against the templated method, assert the sub-goal sequence carries the recipe's specific inputs (Bone + Sinew), NOT a generic "any craft input" set.
- `just soak-trace 42 Simba 900` + `just verdict logs/tuned-42-<sha>/` — no behavior delta vs 461 baseline (substrate widening is dormant; nothing emits `HaveItem`).
- `just frame-diff logs/tuned-42-<461-sha> logs/tuned-42-<462-sha>` shows zero DSE drift (no new L2 emissions yet).
- Substrate-stub canaries (`scripts/check_substrate_stubs.sh`, `check_marker_snapshot_wiring.sh`, `check_method_registry.sh`) green.

## Log

- 2026-05-24: opened as 461's substrate-widening follow-on (blocked-by 461). Rationale and decision history in 461's 2026-05-24 ## Log entry.
- 2026-05-24: Three-commit substrate landing — GoalKind sum type + GoapActionKind::RetrieveCraftInputs(RecipeId) + decompose_goal_have_item helper. Ships dormant per the 462/463 split; 463 lifts the weight.
