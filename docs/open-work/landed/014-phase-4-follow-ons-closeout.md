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

---
