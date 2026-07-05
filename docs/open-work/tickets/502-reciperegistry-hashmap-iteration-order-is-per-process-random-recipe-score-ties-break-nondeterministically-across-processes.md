---
id: 502
title: RecipeRegistry HashMap iteration order is per-process random — recipe score ties break nondeterministically across processes
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Two 900s seed-42 soaks of byte-identical binaries diverge at the first
crafting plan: run A's `PlanCreated` carries
`RetrieveCraftInputs(RecipeId("remedy.mood_tonic"))`, run B's carries
`remedy.healing_poultice` (surfaced at tick 1222651 comparing
`logs/tuned-42-a4dd8a04` vs `logs/tuned-42-25deac3d-run1`; the streams
are byte-identical up to that line, so there is no upstream float
divergence — the flip is the tie-break itself). Root cause:
`RecipeRegistry.recipes` is a `std::collections::HashMap`, whose
iteration order is randomized **per process** (`RandomState`).
`emit_have_item_row` (`aspiration_picker.rs:638`) scans `recipes.iter()`
with first-seen-wins tie-breaking (`score <= bs` keeps the incumbent),
and all three remedy recipes score byte-identically (same
`is_warriors_kit: false`, same `Herbcraft` affinity, no recent-craft
penalty) — so the winner is whichever the process's hash seed yields
first. This violates the determinism doctrine the repo already encodes
in `Relationships` ("BTreeMap, not HashMap, so iteration is stable") and
silently invalidates every byte-identical-stream gate at soak scale. The
600-tick `simulation_is_deterministic` test never reaches crafting
(footer lists `ItemCrafted` in never-fired), so the gate can't see it —
and the two runs it compares live in one process anyway.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Registry storage | `src/resources/recipe_registry.rs:24` | `recipes: HashMap<RecipeId, Recipe>`; `iter()` = `values()` — per-process-random order | `[verified-hostile]` |
| Aspiration emission | `src/systems/aspiration_picker.rs:638-678` | winner scan over `recipes.iter()`, tie keeps first-seen (`score <= bs`); remedy trio scores byte-equal | `[verified-hostile]` |
| First-match lookup | `recipe_registry.rs:52` `recipe_for_item` | `.iter().find(...)` — first match over random order; latent same-bug if two recipes ever share an output kind | `[verified-suspect-latent]` |
| Plan template | `goap.rs` craft chain | RecipeId pinned at emit-time and flows through plan → resolver; resolver (`craft_at_workshop.rs`) takes the id as given — downstream is deterministic given the pick | `[verified-correct]` |
| Determinism gate | `tests/integration.rs::simulation_is_deterministic` | 600 ticks, single process — structurally blind to cross-process HashMap-order effects and to any post-600-tick subsystem | `[verified-correct]` (as far as it goes) |

## Fix candidates

**Parameter-level options:**
- R1 — deterministic tie-break in `emit_have_item_row` only (compare
  `(score, recipe.id)` lexicographically). Fixes the observed site,
  leaves the registry's random order and `recipe_for_item`'s
  first-match latent.

**Structural options:**
- R2 (**rebind storage**) — `RecipeRegistry.recipes` becomes
  `BTreeMap<RecipeId, Recipe>` (derive `Ord` on `RecipeId(&'static
  str)`). Every consumer — winner scans, `recipe_for_item`,
  `craftable_*` — iterates in stable lexicographic id order,
  process-independent. Same fix-shape as the `Relationships` BTreeMap
  precedent (relationships.rs:57-65). Zero per-tick cost change at ≤20
  entries.
- R3 (**retire the map**) — recipes are a compile-time-fixed set;
  store a sorted `Vec<Recipe>` populated once at startup with a
  registration-order contract. Rejected: loses keyed `get(RecipeId)`
  callers and adds an ordering invariant a future insert can silently
  break; BTreeMap gives the same stability with the invariant held by
  the type.

## Recommended direction
R2. The registry is the substrate; pinning order at the storage layer
fixes the observed site and the latent `recipe_for_item` first-match in
one move, with the repo's existing BTreeMap-for-determinism precedent.
R1 alone leaves a landmine the next `.iter().find()` caller steps on.

## Out of scope
- A corpus-wide audit of sim-path `HashMap` iteration for other
  order-dependent consumers (belief maps, near-pair caches are already
  BTreeMap; `integrate_beliefs`' HashMap retains are per-entity-keyed
  state where iteration order only affects float-independent decay).
  If Phase II byte-gates surface another flip, open a sibling with this
  ticket as precedent.
- Extending `simulation_is_deterministic` to cross-process comparison
  (plan.md step 6 already extends it to ≥1000 ticks with position
  traces; a cross-process variant needs a harness change — fold into
  that step if cheap).

## Verification
- Two same-binary 900s seed-42 soaks byte-identical over the common
  tick range (modulo header commit fields + footer wall-clock fields) —
  the exact comparison that surfaced the bug.
- `just check && just test` green; `just verdict` pass (tie now pins to
  lexicographic winner — a one-time behavioral pin, drift carried by
  the verdict gates).

## Log
- 2026-07-05: opened mid-session from the ticket-500 byte-identity gate
  (three-way soak comparison isolated the flip to cross-process
  HashMap order, not the 500 merge-join). R2 recommended.
