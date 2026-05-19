---
id: 373
title: Den/Workshop food retrieval — broaden eat substrate beyond Stores-only
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-16
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Food can physically exist in `Den` and `Workshop` `StoredItems` (cats stash
in Dens; crafting stages raw inputs in Workshops), but the GOAP eat
substrate only knows how to retrieve from `Stores`. The cat-accessible food
pool is narrower than the colony-owned food pool, and the gap is opaque —
no UI surface flagged it until 190's UI work exposed `in_dens` /
`in_workshops` as their own breakdown rows. Concretely:

- `resolve_eat_at_stores` (`src/steps/disposition/eat_at_stores.rs:41-90`)
  filters on `StructureType::Stores` only.
- No sibling resolver exists for Den / Workshop retrieval — grep over
  `src/steps/disposition/` confirms `eat_at_*`, `retrieve_*_from_stores` are
  Stores-keyed.
- The Eat DSE's eligibility (`src/ai/dses/eat.rs:115`) is gated by the
  `HasStoredFood` marker, which is computed from `FoodStores.current` —
  itself Stores-only (preserved that way by 190 for backward compatibility).

So a Den stash of three rabbits is genuinely dark food: tracked by
`FoodStores.in_dens`, displayed to the player, but unreachable to any cat
that didn't deposit it (Inventory-held food they themselves carry is
consumed autonomously via `eat_from_inventory` in `src/systems/needs.rs`).

## Scope

- New step resolver `resolve_eat_at_den` (mirror of `resolve_eat_at_stores`)
  in `src/steps/disposition/`. Targets `StructureType::Den`'s `StoredItems`.
  Five-heading rustdoc preamble per `scripts/check_step_contracts.sh`.
- Extend Eat DSE eligibility (`src/ai/dses/eat.rs`) so the DSE scores when
  EITHER `HasStoredFood` OR a new marker `HasDenFood` is set. Author the
  `HasDenFood` marker in `src/components/markers.rs` + writer alongside
  `update_colony_building_markers` in `src/systems/buildings.rs`.
- Extend the Eat plan template (`src/ai/planner/actions.rs:172-182`) to
  emit a Den-retrieval plan when the chosen target is a `Den`.
- Substrate-stub discipline: marker + writer ship in the same commit per
  `scripts/check_substrate_stubs.sh`. Wire into
  `populate_dse_registry`-equivalent registration if needed.

## Out of scope

- **Workshop retrieval.** Workshop-staged raw food is mid-craft; cats
  retrieving from it would race the Cook DSE for inputs. Either resolve
  this with explicit cross-DSE arbitration or leave Workshop staging as a
  short-lived intermediate (default). Park for a separate ticket.
- **Cross-cat handoff from Dens.** Cats taking from another cat's Den isn't
  the same operation as Stores (which are shared). Social model is unclear
  — does ownership matter? Park.
- **Caching `HasDenFood` in `ColonyState`.** The marker authorship can use
  the same scan-StoredItems pattern as `has_raw_food_in_stores`; perf
  optimization is a follow-on.
- **Tuning `build_chronic_full_weight` (ticket 190).** Independent — the
  chronic-full latch is about deposit rejection on Stores, not about Den
  retrieval. 190 proceeds in parallel.

## Current state

Discovered during ticket 190's UI scope expansion (2026-05-16). 190 landed:

- `FoodStores.in_stores` / `in_dens` / `in_workshops` / `held` breakdown
  fields (`src/resources/food.rs`).
- `sync_food_stores` extended to count food across all four sources
  (`src/systems/items.rs:155-260`).
- UI breakdown row in ResourcePanel showing
  `Total X food (Y stores · Z dens · ...)`.

These are passive observability. This ticket adds the actuator.

## Approach

Mirror the Stores-eating substrate to a sibling Den-eating path:

1. **L1 marker.** `HasDenFood` on `ColonyState`. Writer in
   `update_colony_building_markers` scans `StructureType::Den` `StoredItems`
   for any food item (same shape as the existing `has_raw_food_in_stores`
   scan at `src/systems/buildings.rs:495-502`). Test: insert/remove parity
   with a den containing/lacking food.

2. **L2 DSE eligibility.** Extend `EatDse` to accept `HasStoredFood OR
   HasDenFood`. The existing MarkerConsideration / curve / weights should
   carry over — Den food isn't more or less attractive than Stores food
   modulo distance, which the spatial consideration already prices.

3. **L3 plan template.** When the planner picks Eat, target selection
   currently produces a Stores entity. Extend to enumerate
   `Stores ∪ Dens` candidates, score by distance + food presence, pick
   nearest. Plan template branches by target's `StructureType` to call
   either `resolve_eat_at_stores` or `resolve_eat_at_den`.

4. **Resolver.** `resolve_eat_at_den` is structurally identical to
   `resolve_eat_at_stores` — different building filter, same item
   consumption + needs.hunger restoration + `Feature::FoodEaten` emission.

## Verification

- Scenario microexperiment (preferred over soak per CLAUDE.md): write a
  `den_food_retrieval` scenario under `src/scenarios/` that spawns a hungry
  cat next to a Den containing a mouse, no Stores. Assert: Eat DSE wins,
  plan resolves to Den, food consumed, hunger restored, `Feature::FoodEaten`
  fires. ~3s feedback loop.
- Survival hard-gates: `just verdict` on a soak after this lands —
  Starvation == 0, ShadowFoxAmbush ≤ 10, never-fired-positives == 0, all
  five continuity canaries firing.
- Frame-diff vs the 190-promoted baseline. Expectation: Eat action share
  unchanged or slightly higher (Den food becomes accessible), Forage / Hunt
  unchanged (cats not driven to gather more), `colony_score.aggregate`
  unchanged or up.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **189** (done, ai-substrate, score 0.88) — Post-178 food_available regression — layer-walk diagnosis
- ✓ landed ** 94** (done, substrate-over-override, score 0.88 (cross-cluster)) — Eat-vs-Forage IAUS imbalance — colony hauls food but doesn't consume it
- ✓ landed ** 91** (done, ai-substrate, score 0.88) — "Post-087 seed-42 plan-execution collapse — Eat picks 62% but FoodEaten never w…

<!-- linkages:end -->
## Log

- 2026-05-16: opened as 190's land-day follow-on. 190's UI work surfaced
  the dark-food gap visually; this ticket closes it by making Den-stashed
  food retrievable. Workshop retrieval parked separately (mid-craft race).
- 2026-05-19: accuracy audit pass — blocked-by empty and status ready; referenced systems exist (ai-substrate-refactor.md); related-work 189/94/91 landed
