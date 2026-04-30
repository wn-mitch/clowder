---
id: 048
title: "Phase 2C CarcassScentMap, the §5.6.3 #6 influence map"
status: done
cluster: null
landed-at: 405740b7
landed-on: 2026-04-27
---

# Phase 2C CarcassScentMap, the §5.6.3 #6 influence map

**Landed:** 2026-04-27 | **Commit:** `405740b7`

**Why:** Spec §5.6.3 row #6 commits a Carcass-location influence map on (`Channel::Scent`, `Faction::Neutral`) backing `ScoringContext`'s `carcass_nearby` and `nearby_carcass_count` fields. Pre-landing those fields populated via per-pair `observer_smells_at` ECS iteration in `goap.rs:1133–1145` — there was no shared persistent map. This brick adds one, advancing the §5 substrate one absent-row at a time after Phase 2B's PreyScentMap landing.

**Scope (substrate-only):** new `CarcassScentMap` resource (bucketed `f32` grid, mirror of `PreyScentMap`), per-tick `carcass_scent_tick` system in `src/systems/wildlife.rs:805+` (decay-then-deposit ordering, actionable filter `!cleansed || !harvested` matching the existing `goap.rs:840–846` snapshot), `InfluenceMap` impl, registered in `SimulationPlugin` + `setup.rs`, walked by the trace emitter. Two new `WildlifeConstants` knobs — `carcass_scent_deposit_per_tick: 0.1` (mirrors prey) and `carcass_scent_decay_rate: RatePerDay::new(0.5)` (slow fade per §5.6.5 #6) — both `#[serde(default)]` for events.jsonl back-compat.

**What did NOT land:** consumer cutover. `goap.rs:1133–1145` still uses `observer_smells_at` per-pair detection. The §6.3 cutover that would replace those reads with `carcass_scent_map.get(pos)` is a separate balance-affecting follow-on. Predicted shift on `CarcassHarvested` (baseline mean 6.3, 12/15 sweep runs): rises toward consistent firing across 15/15. That landing requires a four-artifact soak per `CLAUDE.md`. Phase 2D registry refactor of `trace_emit.rs:120+`, kitten-urgency map (#13), corruption full migration off `CorruptionLens`, wards #3 promotion, and per-prey-species split of `PreyScentMap` (#5) are also deferred.

**Verification:** `just check` clean. `cargo test --lib` 1359/1359 passed (6 new on `carcass_scent_map`). `just soak 42` survived; canary footprint matches the immediate-prior post-045 commit one-for-one (Starvation == 0, ShadowFoxAmbush == 4 ≤ 10, footer written, never_fired list identical: `[FoodCooked, MatingOccurred, GroomedOther, MentoredCat, CourtshipInteraction]`). The `verdict` `fail` status is the pre-existing condition the ticket-045 landing commit message explicitly anticipated. 30s focal-cat trace (`--focal-cat Simba --duration 30`) emitted 5770 `carcass_scent` L1 records with correct attenuation (`species_sens=1.0` for Cat × Scent), faction `neutral`, channel `scent`, name `carcass_scent`. Wiring confirmed.

**Files:** `src/resources/carcass_scent_map.rs` (new, 183 LOC), `src/systems/wildlife.rs::carcass_scent_tick` (new, 38 LOC), `src/systems/influence_map.rs` (+17 LOC `InfluenceMap` impl), `src/systems/trace_emit.rs` (+19 LOC param + walk + docstring), `src/resources/sim_constants.rs` (+23 LOC two knobs + defaults), `src/resources/mod.rs` (+2 LOC export), `src/plugins/setup.rs` (+4 LOC resource insert), `src/plugins/simulation.rs` (+1 LOC system register). Wiki + open-work index regenerated.

**Net behavioral delta:** zero. The map populates and is observable via traces but does not yet drive scoring.

---
