---
id: 014
title: "§4.2 State marker trio — `InCombat` / `OnCorruptedTile` / `OnSpecialTerrain` authors"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-25
---

# §4.2 State marker trio — `InCombat` / `OnCorruptedTile` / `OnSpecialTerrain` authors

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

---
