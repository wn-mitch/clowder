---
id: 062
title: Prey-species split — per-species scent maps (§5.6.3 row #5)
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

§5.6.3 row #5 calls for per-prey-species sight maps (mouse / rat /
rabbit / fish / bird). Today `PreyScentMap` is one aggregate map
across all prey species; ticket 048 landed the substrate and explicitly
deferred the per-species split.

Per-species discrimination matters for two cases the aggregate map
masks:
1. **Dietary specialization** — a cat with a bird-hunting tradition
   (or fox stalker tradition) would prefer to point itself toward
   *its* species's scent gradient, not whichever prey happens to
   smell strongest.
2. **Ward / fox-deterrent placement** — a prey-rich tile near a den
   might deserve a different ward than one that's just bird-rich.

Inherits from ticket 048's "Phase 2C — Carcass scent map landed,
per-prey-species split deferred" log entry.

## Scope

1. Replace `PreyScentMap` with a small enum-keyed map registry
   (one map per `PreyKind`: `Mouse`, `Rat`, `Rabbit`, `Fish`, `Bird`).
   Keep the existing struct shape (bucketed grid, per-tick decay) —
   only the keying changes.
2. **`InfluenceMap` impls** for each — `metadata().name`:
   `"prey_scent_mouse"`, `"prey_scent_rat"`, etc. Faction tagged as
   `Faction::Species(SensorySpecies::Wild(<kind>))`.
3. **Writer cutover** in `src/systems/prey.rs::prey_scent_tick` —
   single pass over `Query<(&PreyAnimal, &Position)>` dispatches the
   deposit to the correct per-species map by `PreyAnimal::kind`.
4. **Existing aggregate consumer cutover** — `Hunt` / `Hunting` (cat +
   fox) DSEs that today read `prey_scent` switch to either:
   - **A specific species map** if the DSE has a species preference
     (e.g. a hunting-tradition cat reads only bird).
   - **A composite read** (`max` or `sum` clamped) across all species
     for the species-agnostic case.
5. **Backward compat** — the aggregate `prey_scent` map_key may stay
   alive as a derived view (computed on read by max-aggregating across
   per-species maps) so consumers that haven't cut over yet continue to
   work. Decision: drop the aggregate or keep the lens? Implementation
   call.

## Out of scope

- Cross-species prey-selection scoring tuning (balance work; lives in
  the post-cutover balance thread once consumers are wired).
- Per-cat hunting-tradition memory persistence (different ticket).

## Verification

- Lib tests on each per-species `InfluenceMap` impl (faction tagged
  correctly, sample reads from the right map).
- Soak verdict on canonical seed-42 deep-soak after landing — must
  return exit 0.
- Per-DSE drift check via `just frame-diff` — Hunt / Hunting score
  shifts must be either behavior-neutral (if the species-agnostic
  composite preserves the old aggregate read) or hypothesis-required
  (with concordance check per balance methodology).
- Focal trace: `just soak-trace 42 Simba` — confirm
  `prey_scent_*` per-species samples appear in L1 records.

## Log

- 2026-04-27: opened from ticket 006 closeout. Inherits the deferral
  ticket 048 logged when carcass-scent landed.
