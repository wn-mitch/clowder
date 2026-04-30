---
id: 014
title: §4 marker author systems batch 1
status: done
cluster: null
landed-at: null
landed-on: 2026-04-24
---

# §4 marker author systems batch 1

**What shipped:**

- 5 new KEY constants: `Injured`, `IsCoordinatorWithDirectives`,
  `HasHerbsInInventory`, `HasRemedyHerbs`, `HasWardHerbs`.
- 3 per-cat ECS marker author systems:
  - `needs::update_injury_marker` — any unhealed injury (broader than
    Incapacitated). Prerequisite for Capability markers.
  - `items::update_inventory_markers` — HasHerbsInInventory, HasRemedyHerbs,
    HasWardHerbs via Inventory helper methods.
  - `coordination::update_directive_markers` — IsCoordinatorWithDirectives
    with non-coordinator cleanup query.
- Colony-scoped marker helpers (DRY, not full author systems):
  - `buildings::scan_colony_buildings` — single-pass HasGarden,
    HasFunctionalKitchen, HasConstructionSite, HasDamagedBuilding.
  - `magic::is_ward_strength_low` — WardStrengthLow predicate.
  - Both goap.rs and disposition.rs cutover to shared helpers,
    eliminating ~30 lines of duplicated predicate logic each.
- `MarkerQueries` SystemParam extended with `per_cat` query for
  5 new Has<M> booleans; MarkerSnapshot population reads from
  authored ZSTs instead of inline computation.
- ScoringContext fields `has_herbs_in_inventory`, `has_remedy_herbs`,
  `has_ward_herbs`, `is_coordinator_with_directives` now populated
  from authored markers via MarkerSnapshot.
- Coordinate DSE gains `.require("IsCoordinatorWithDirectives")`
  on its EligibilityFilter; inline `if ctx.is_coordinator_with_directives`
  guard retired in scoring.rs.
- 31 new tests across 5 modules.
- Inventory gains `has_any_herb()` method (`components/magic.rs`).
- Chain 2a registration at both schedule sites (simulation.rs + main.rs).

**Deferred:**

- ColonyState singleton spawn + real ZST markers on it — the colony
  predicates use shared helpers into MarkerSnapshot for now. Singleton
  promotion is a follow-on.
- HasRawFoodInStores stays inline (CookingQueries already encapsulates
  the stored-items predicate). Helper extraction deferred.
- ScoringContext field removal — fields retained for non-scoring
  consumers. Full removal in a future cleanup pass.
- Capability markers (CanHunt, CanForage, CanWard, CanCook) depend
  on Injured; now unblocked, targeted for batch 2.

---
