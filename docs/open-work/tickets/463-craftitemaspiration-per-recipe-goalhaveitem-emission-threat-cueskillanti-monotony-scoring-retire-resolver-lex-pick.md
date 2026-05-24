---
id: 463
title: "CraftItemAspiration: per-recipe Goal(HaveItem) emission + threat-cue/skill/anti-monotony scoring + retire resolver lex pick"
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-24
parked: null
blocked-by: [462]
supersedes: []
related-systems: [crafting.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

461's design review (2026-05-24) established that recipe selection in crafting should be desire-driven — a cat aspires to a specific item, that aspiration emits `Intention::Goal(HaveItem(<ItemKind>))` into the L2 softmax pool, and the HTN method registry decomposes it into a craft chain. The substrate widening lands in 462 (`GoalKind` enum + templated method + parameterized `RetrieveCraftInputs`). 463 lifts the dormant weight: a `CraftItemAspiration` chain that scores satisfiable recipes per cat per tick and emits the typed Goals that 462's substrate is waiting for.

Once 463 lands, cats actually craft warrior's-kit items in the seed-42 soak (the gate that returned zero on 369's first-light), the L2 trace explicitly carries `have_<item>` labels for the winning Intention (per the L2-trace pillar — the choice is finally legible), and the resolver-internal lex pick at `src/steps/disposition/craft_at_workshop.rs:139-153` can retire (or stay as a non-aspiration fallback — design call at implementation time).

## Scope

- **New `CraftItemAspiration` chain** under `src/ai/aspirations/`. Per-cat per-tick loop: for each recipe in the registry, check **per-cat per-recipe** input accessibility — `recipe.inputs` all reachable to *this cat* (in inventory OR in a Stores aggregate this cat can walk to). For recipes that pass, emit one `Intention::Goal(GoalState { kind: GoalKind::HaveItem(recipe.output) })` into the L2 pool, scored by:
  - `+threat_belief * recipe.is_warriors_kit` (read site: `src/components/beliefs.rs:149`, fed by Attack / FleeFrom / AmbientShock via `belief_integrator.rs:185-473`).
  - `+skill_match * recipe.discipline_skill_affinity` (new recipe metadata field — see below).
  - `-recent_use_bonus / (1 + ticks_since_last_craft_of_this_id)` (new per-cat `CatRecentCrafts` Component — ring buffer of last-N `RecipeId` + tick stamps).
  The per-cat per-recipe accessibility gate is the substrate replacement for the generic colony marker the original 461 plan called for — selection lives in the aspiration, not in a marker.
- **Recipe-registry metadata**: add `is_warriors_kit: bool` and `discipline_skill_affinity: Option<SkillKind>` to recipe records in `src/components/recipe.rs` (verify the exact path during implementation). 8 warrior's-kit recipes from 369 get `is_warriors_kit: true`; behavioral-tool recipes from 368 get appropriate skill affinities.
- **New `CatRecentCrafts` Component** under `src/components/`. Ring buffer of `(RecipeId, tick)` pairs, fixed size (4-8 — author's call). Updated by `resolve_craft_at_*` on successful craft. Used by the aspiration's anti-monotony score component.
- **Retire resolver lex pick.** `resolve_craft_at_workshop` / `resolve_craft_at_tanning_frame` become primitives — they no longer pick a recipe; they receive `RecipeId` as a parameter from the HTN-decomposed plan step (the templated method named the recipe at decomposition time, derived from the held Intention's `HaveItem` variant). `pick_satisfied_recipe` (`src/steps/disposition/craft_at_workshop.rs:139-153`) is deleted, or kept as a non-aspiration fallback if the 463 author wants belt-and-braces — but the typed-Goal flow is the canonical shape.

## Out of scope

- **DryFood / Cook conversion** to typed Goals (open a 4th sibling against 462 once 463 lands).
- **Slot-aware equipment semantics** (depends on 017).
- **Resolver reads of material-property effects** (hunt-strike weapon-bonus, take_damage armor-reduction, ranged-attack sling-enable, movement-detection cloak-mask, noise-class detection-penalty) — separate follow-on now that 463 makes kit items actually exist.
- **`Feature::BoneWeaponSnapped` emitter** — lands with the resolver-reads follow-on (snap is gated on failed-strike).
- **`CraftInPlaceDse`** for open-ground knapping — separate clean-up; 369 routed `flint_blade` through Workshop as a compromise.

## Current state

- 461 lands TanningFrame threshold tuning (independent — gives 463 somewhere to craft hide gear).
- 462 lands the substrate widening (`GoalKind::HaveItem`, templated HTN method, parameterized `RetrieveCraftInputs`).
- 463 is dormant-substrate-activation: lifts a 0.0 weight off the new substrate.
- `belief_integrator` already produces threat-cue facets (`src/components/beliefs.rs:149`) — no upstream wiring needed.
- Per-cat per-recipe accessibility check uses existing `recipe.inputs` access pattern from `pick_satisfied_recipe` — same lookup logic, evaluated at aspiration-emit time rather than resolver time.
- Existing `populate_aspiration_registry` (or equivalent) under `src/ai/aspirations/` provides the registration shape. `RaiseOffspringAspiration` is the precedent for "aspiration emits typed Goal Intentions into the unified softmax pool."

## Approach

1. **Recipe metadata first.** Add `is_warriors_kit` + `discipline_skill_affinity` to recipe records. Populate for all current recipes. `just check` green.
2. **`CatRecentCrafts` Component.** Add the ring buffer. Add insert site in the craft resolvers (idempotent — `(recipe_id, tick)` on success). Initial empty for all cats. `just check` + `cargo test` green.
3. **`CraftItemAspiration` chain skeleton with scoring weight 0.0.** Registers in the aspiration registry. Emits typed Goals using 462's substrate. Verified by focal-trace showing the new `have_<item>` Intentions emit with score 0.0 (visible in L2 trace, doesn't compete with other DSEs). This is the "first-light activation" per memory `feedback_dormant_substrate_activation_soak_first` — single `just soak-trace` answers "does the layer fire?" before any tuning.
4. **Lift the weight.** Tune threat-cue weight, skill-match weight, anti-monotony coefficient. Per-soak iteration until the variation gate fires.
5. **Retire `pick_satisfied_recipe`** once the aspiration path is producing all crafts. Delete or fence behind a fallback path.
6. **First-light soak** `just soak-trace 42 Simba 900` (+ a second focal-trace on an Adult cat with high threat-belief to verify warrior's-kit-specific axes — per CLAUDE.md "Multi-focal convention" for marker-gated DSEs). Verdict gates.

Note (per memory `feedback_park_demographic_dependent_tuning`): 463's emit gate is per-cat per-recipe input-accessibility, which depends on Stores having warrior's-kit prey-byproducts. If 369's prey-rabbit hunt cadence is too low to supply Bone / Sinew / Whisker / Hide at the rate 463's aspiration needs to fire, this ticket parks behind a prey-cadence ticket. Check first soak's `events.jsonl` for prey-byproduct deposit counts before drafting tuning iterations.

## Verification

- `just check` + `cargo test --release --lib` green.
- `just soak-trace 42 Simba 900` + `just verdict logs/tuned-42-<sha>/` — verdict pass.
- **Variation gate**: ≥3 distinct warrior's-kit ItemKinds produced in the 900-tick soak. `grep -E '(BoneStiletto|BoneTipSpear|FlintBlade|HideBracers|HidePlatedWrap|Sling|WovenReedCloak|ToothNotchedClub)' logs/tuned-42-<sha>/events.jsonl | sort -u | wc -l` returns ≥ 3.
- **TanningFrame product gate**: ≥1 hide-gear item built (`HideBracers` or `HidePlatedWrap` or `WovenReedCloak`) — proves the TanningFrame from 461 had a consumer.
- `just frame-diff logs/tuned-42-<462-sha> logs/tuned-42-<463-sha>` shows new per-recipe `have_<item>` rows in L2; no wrong-direction drift on existing DSEs.
- L2 trace explicitly carries `have_<item>` labels for the winning Intention (per the L2-trace pillar — the choice is now legible).
- Continuity canaries hold or improve vs the 462 baseline.

## Log

- 2026-05-24: opened as 462's aspiration-activation follow-on (blocked-by 462). Rationale and decision history in 461's 2026-05-24 ## Log entry.
