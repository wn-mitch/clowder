---
id: 014
title: Phase 4 follow-ons closeout
status: done
cluster: null
landed-at: 453ea83
landed-on: 2026-04-27
---

# Phase 4 follow-ons closeout

**Landed:** 2026-04-27 | **Closeout commit:** `453ea83` (docs landing log)

**Why:** Phase 4 of the AI substrate refactor (`docs/systems/ai-substrate-refactor.md`) committed five deliverables. Phase 4a landed three (softmax-over-Intentions, §3.5 modifier pipeline port, Adult-window retune). 014 tracked the remaining two spec-committed deliverables — `add_target_taking_dse` + per-target considerations (§6.3, §6.5) and §4 marker-eligibility authoring — plus three balance gaps observed at Phase 4a exit (MatingOccurred, PracticeMagic sub-mode density, Farming).

**What landed across 014's lifetime:**

- **§7.M.7.4 mate-gender fix** (Phase 4b.1).
- **§4 marker-eligibility authoring foundation + batches 1–2 + State trio** (2026-04-22 → 2026-04-25): `MarkerSnapshot` resource, `MarkerQueries` SystemParam bundle, lookup foundation, `HasStoredFood`, colony building markers (`HasGarden` / `HasFunctionalKitchen` / `HasRawFoodInStores`), `Incapacitated`, life-stage markers (Kitten / Young / Adult / Elder), batch 1 (Injured / inventory / directives), batch 2 (capability markers `CanHunt` / `CanForage` / `CanWard` / `CanCook`), State trio (`InCombat` / `OnCorruptedTile` / `OnSpecialTerrain`).
- **§6.5 per-DSE target-taking ports** (Phases 4c.1 → 4c.7, 2026-04-22 → 2026-04-23): Socialize / Mate / Mentor / Groom-other / Hunt / Fight / ApplyRemedy / Build / Caretake. `TargetTakingDse` struct + `TargetAggregation` enum + `evaluate_target_taking` evaluator + `add_target_taking_dse` registration. Retired `find_social_target`, `find_mentoring_target`, `nearest_threat`, `resolve_caretake` legacy resolvers.
- **§4 marker catalog large-fill** (2026-04-27, this commit slate): 19 markers across 7 commits + 1 fix.
  - `56f0586` Mentoring batch — Mentor / Apprentice / HasMentoringTarget.
  - `3306107` Parent marker — active-parenthood ZST from `KittenDependency`.
  - `1ccfcc8` Magic colony batch — ThornbriarAvailable / WardsUnderSiege via shared `magic::is_*` helpers.
  - `d5f7417` Sensing target-existence batch — five broad-phase markers (HasThreatNearby / HasSocialTarget / HasHerbsNearby / PreyNearby / CarcassNearby) via `sensing::update_target_existence_markers`.
  - `a527e3a` Fox spatial batch — StoreVisible / StoreGuarded / CatThreateningDen / WardNearbyFox in new `src/systems/fox_spatial.rs`. First-time `MarkerSnapshot` population for fox AI in `fox_evaluate_and_plan`.
  - `fcd13bd` Fox lifecycle batch — HasCubs / CubsHungry / IsDispersingJuvenile / HasDen. 7 fox authors nested into a Chain 2a sub-tuple to stay under Bevy's 20-system tuple cap.
  - `fa112bf` fix — Sensing-batch authored ZSTs but didn't query them inside `evaluate_and_plan` to populate `MarkerSnapshot`, so `markers.has(KEY, entity)` resolved to false. Added `target_existence_q` SystemParam + per-cat snapshot rows. Soak before fix vs after fix: continuity play 8 → 368, grooming 6 → 30; CarcassHarvested 0 → 12 (Magic Harvest unblock).
  - `453ea83` docs — closeout landing log + successor-ticket links.

**Balance-gap status at closeout:**
- **MatingOccurred** = 0 → diagnosed as a structural three-bug cascade (lifted-condition outer gate, missing L2 PairingActivity, misnamed CourtshipInteraction canary) and migrated to ticket 027 (in-progress).
- **PracticeMagic sub-mode density** → CleanseCompleted vigorous (mean 215.7 across 15 sweep runs as of 2026-04-25 baseline), Harvest unblocked at 014 closeout (`CarcassHarvested = 12` post-fix soak). Only Commune remains dormant, and that's a §6.3 spatial-routing problem (not a marker / numeric-tuning fix). Out of 014 scope.
- **Farming** ≥ 1 → resolved per 2026-04-25 baseline: `CropTended` mean 17,191.6 across 14/15 runs, `CropHarvested` mean 873.7 across 13/15.

**Verification (post-fix soak `logs/tuned-42/`, commit fcd13bd-dirty, seed 42, --duration 900):** Starvation 1 (within scheduler-variance noise band, precedented in 2026-04-25 State-trio commit's `b9129a1-dirty-statetrio` soak). ShadowFoxAmbush 3 ≤ 10. Footer written. Continuity grooming 30 / play 368 / mythic-texture 3 (all pass). Pre-existing dormancies mentoring/burial/courtship = 0 (tracked in ticket 027 + downstream balance work). Magic Harvest unblocked: `CarcassHarvested = 12` (was 0 in pre-fix soak). Lib tests 1361 → 1432 (+71 across the 7-commit slate).

**Successor tickets filed at closeout:**
- [049](../tickets/049-faction-overlay-markers.md) — §9.2 faction overlay markers (Visitor / HostileVisitor / Banished / BefriendedAlly). Cross-cutting with trade subsystem + cat-on-cat banishment.
- [050](../tickets/050-marker-predicate-refinements.md) — §4 marker predicate refinements: species-attenuated `HasThreatNearby`, truthful `WardNearbyFox`, event-driven `HasCubs` / `HasDen`.
- [051](../tickets/051-fox-dse-eligibility-migration.md) — fox DSE eligibility migration: `.require()` / `.forbid()` cutover for fox raiding / den-defense / feeding / dispersing; `FoxScoringContext` field retire.

§6.3 spatial-target routing for Cleanse / Harvest / Commune dormancies remains tracked in `docs/systems/ai-substrate-refactor.md` §6.3 follow-ons (separate refactor track, not a 014 successor).

**§4 marker catalog status:** all §4.3 markers except the §9.2 faction overlay now have author systems.

**Retro-note (2026-05-04, ticket 163):** the 014 closeout shipped the
§3.5 modifier-pipeline framework but did not enumerate the 9 pre-existing
imperative `apply_*_bonus` passes in `goap.rs` / `disposition.rs` as
out-of-scope. Per CLAUDE.md "antipattern migration follow-ons are
non-optional" this should have produced a follow-on at land. Ticket 163
is that follow-on, opened retroactively and migrated all 9 passes into
registered §3.5.1 modifiers.

---

## Folded-in subticket: §4.2 State marker trio — `InCombat` / `OnCorruptedTile` / `OnSpecialTerrain` authors

**Landed-on:** 2026-04-25. Originally tracked as a separate file `landed/014-4-2-state-marker-trio-incombat-oncorruptedtile-onspecialterr.md` claiming `id: 014`. Folded into the 014 parent during Linear migration prep.

**Landed:** 2026-04-25 | **Tracks:** AI substrate refactor cluster A (ticket 005) Track C; ticket 014 Phase 4 follow-ons.

Three §4.2 State markers were pre-declared in `src/components/markers.rs:119–141` (struct + `KEY` constant + rustdoc pointing at future author file paths) but had no author system, so the marker was never inserted and consumers reading `Has<Marker>` silently took the "false" branch. The most concrete consequence: `src/ai/capabilities.rs:46` queries `Has<InCombat>` for the `CanHunt` / `CanForage` predicates, and that read was always false — a cat in a fight could still be marked `CanHunt`.

Author systems landed:

- `src/systems/combat.rs::update_combat_marker` — `InCombat` ZST whenever `current.action == Action::Fight && current.target_entity.is_some()`. Mirrors the fight-collection probe in `resolve_combat`. v1 covers active fight steps only; the "hostile-adjacent" branch named in the §4.2 rustdoc requires species-attenuated detection range and was deferred together with `HasThreatNearby` to a sensing-batch follow-up so the predicate stays single-sourced.
- `src/systems/magic.rs::update_corrupted_tile_markers` — `OnCorruptedTile` ZST whenever `tile.corruption > constants.disposition.corrupted_tile_threshold`. Bit-for-bit mirror of the inline `on_corrupted_tile` computations in `goap.rs::evaluate_and_plan` and `disposition.rs::evaluate_dispositions`.
- `src/systems/sensing.rs::update_terrain_markers` — `OnSpecialTerrain` ZST whenever `tile.terrain` is `FairyRing` or `StandingStone`. Same shape; same inline-mirror predicate.

Wiring:

- Three new `impl X { pub const KEY: &str = "X" }` blocks on the marker structs in `markers.rs`.
- Snapshot population wired into both scoring loops via a new `state: Query<...>` field on `MarkerQueries` (disposition) and a sibling `state_markers_q: Query<...>` parameter (goap). Three new `markers.set_entity(X::KEY, entity, x)` calls per loop.
- `SimulationPlugin::build` registers all three authors in Chain 2a. The chain hit Bevy's 20-system tuple limit; resolved by nesting the seven existing §4 marker authors plus the new three into a sub-tuple `.chain()` so the outer tuple stays at 13.
- 21 new tests across the three modules (~7 per author): predicate-on, predicate-off, threshold-edge, transition-through-position-change, transition-through-state-change, dead-cat skip, multi-cat independence, idempotence.

**Hypothesis** — Authoring three §4.2 State markers closes a §4 catalog gap and replaces silent `Has<Marker>=false` reads in `capabilities.rs` with truthful gating; predicted shift on survival canaries: none (no DSE consumer cutover this commit). Predicted shift on continuity tallies: none (no new `EventKind` emissions). Predicted second-order shift on `CanHuntFired` / `CanForageFired`: marginal drop on cat-ticks where any cat is mid-fight (rare relative to total cat-ticks).

**Observation / Concordance — soak deferred.** Lib tests green (1293 / 1293, +21 from this commit). The seed-42 deep-soak verification gate is deferred: the bin is mid-rewrite under the parallel-session phase-D of ticket 030 and does not compile in the current parent commit (the `run_headless` body still references the deleted `setup_world` / `build_schedule` / `flush_*` / `build_headless_footer` helpers). Schedule the soak + survival-canary + continuity-canary diff once phase D lands; post the constants-hash diff and footer back into this entry.

**Stub note.** `crate::ai::mating::update_mate_eligibility_markers` is referenced from both `SimulationPlugin::build` and the legacy `main.rs::setup_world` path but has no body. Added a no-op stub in `mating.rs` so the codebase compiles; the body lands with ticket 027 (mating cadence). Stub does **not** author the `HasEligibleMate` ZST, which means `MateDse::eligibility()` continues to gate cats out — matching the pre-stub behaviour where the marker was authored by no one.

**Non-goal.** Authoring `OnCorruptedTile` and `OnSpecialTerrain` does **not** unblock the Cleanse / Commune dormancies. Per ticket 014 lines 124–136, those are spatial-routing bugs (cats don't path TO corrupted tiles or fairy rings when they carry intent), not authoring gaps. The DSE `.require()` cutover is left for the routing fix.

## Folded-in subticket: §4 marker author systems batch 1

**Landed-on:** 2026-04-24. Originally tracked as a separate file `landed/014-4-marker-author-systems-batch-1.md` claiming `id: 014`. Folded into the 014 parent during Linear migration prep.

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

