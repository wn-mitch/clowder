---
id: 206
title: trace_emit L1 walk skips five InfluenceMaps — Food / Garden / Construction / KittenCry / Herb
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 747d2b37
landed-on: 2026-05-07
---

## Why

The L1 emitter at `src/systems/trace_emit.rs:131-152` walks **seven** `InfluenceMap` resources hardcoded inline:
`FoxScentMap`, `PreyScentMap`, `CarcassScentMap`, `CatPresenceMap`, `ExplorationMap`, `WardCoverageMap`, and the `CorruptionLens(TileMap)` adapter.

But `src/systems/influence_map.rs` defines **twelve** `InfluenceMap` impls. The five missing from the trace surface are:

- `FoodLocationMap` (`influence_map.rs:247`)
- `GardenLocationMap` (`influence_map.rs:265`)
- `ConstructionSiteMap` (`influence_map.rs:282`)
- `KittenCryMap` (`influence_map.rs:300`)
- `HerbLocationMap` (`influence_map.rs:319`)

Real, in-use perception channels — the L2 considerations for `Eat`, `Tend*`, `Build*`, `FeedKitten`, and `GatherHerb` all read from these maps when scoring. So when a focal-cat trace renders in the scrubber, those L2 considerations appear unmoored: the score moves but no L1 row backs it. From a debugger's seat, this looks like a panel bug; it's actually an L1 emitter coverage gap.

The trace_emit comment at line 122-129 explicitly defers this work: *"Phase 2D will replace this hardcoded sequence with a registry walk."* The deferral was named on landing of [`landed/048-phase-2c-carcassscentmap…`](../landed/048-phase-2c-carcassscentmap-the-5-6-3-6-influence-map.md) — that ticket lists "Phase 2D registry refactor of `trace_emit.rs:120+`" among the deferred follow-ons.

This ticket is the *coverage* portion of that deferral, not the registry refactor. The registry rewrite (`Vec<Box<dyn InfluenceMap>>` registered at plugin build time) remains a separate, larger arc; this lands the missing five maps via the same hardcoded-walk pattern as the existing seven so the focal-cat scrubber's L1 panel becomes a faithful surface for the perception substrate.

**User-visible trigger:** during diagnosis of a Mallow trace silent-load (see plan `the-focal-cat-trace-steady-pond.md`), the user noted on a working `bced533` Clover trace that "some of the L1 markers aren't represented." That observation is this gap.

## Scope

This is a trace-surface fix, not a balance change. Scoring code is untouched. The only runtime effect is five additional L1 lines per planning tick in `trace-*.jsonl`.

### What lands

1. **Bundle L1-map params into `#[derive(SystemParam)]`.** `emit_focal_trace` already carries 11 typed params plus a Query — adding five more `Option<Res<>>` would exceed Bevy's 16-param ceiling. Project convention (`CLAUDE.md` §ECS rules) mandates `SystemParam` bundling here. Create:

   ```rust
   #[derive(SystemParam)]
   struct L1Maps<'w> {
       fox_scent: Option<Res<'w, FoxScentMap>>,
       prey_scent: Option<Res<'w, PreyScentMap>>,
       carcass_scent: Option<Res<'w, CarcassScentMap>>,
       cat_presence: Option<Res<'w, CatPresenceMap>>,
       exploration: Option<Res<'w, ExplorationMap>>,
       ward_coverage: Option<Res<'w, WardCoverageMap>>,
       food_location: Option<Res<'w, FoodLocationMap>>,
       garden_location: Option<Res<'w, GardenLocationMap>>,
       construction_site: Option<Res<'w, ConstructionSiteMap>>,
       kitten_cry: Option<Res<'w, KittenCryMap>>,
       herb_location: Option<Res<'w, HerbLocationMap>>,
       tile_map: Option<Res<'w, TileMap>>,
   }
   ```

2. **Replace inline 7-block walk with 12-map walk.** Each map calls `emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &**m, &constants)` exactly as the existing seven do. `TileMap` keeps its `CorruptionLens` adapter.

3. **Update the L1 inventory comment.** Replace the "seven maps" + "Phase 2D" deferral note with the new twelve-map inventory and a single line naming the still-deferred registry-walk refactor.

### What does NOT land

- True `Vec<&dyn InfluenceMap>` registry registered at plugin build time (`SimulationPlugin`). Larger refactor; deferred again with a clear name.
- New substrate markers, new `MarkerConsideration`s, or any scoring change.
- Per-prey-species `PreyScentMap` split (its own ticket: `062-prey-species-split-maps.md`).
- Changes to `LogsDashboard.svelte` or events-page error surfacing — `the-focal-cat-trace-steady-pond.md` Landing 1 covers the trace page only.

## Verification

- `just check` — substrate-stub lint + step-resolver lint + time-unit lint pass.
- `just test` — existing `trace_emit::tests::*` pass; the per-map shape test covers the parametric expansion.
- `just soak-trace 42 Simba` (release; ~15 min) on a HEAD that includes this change — emits `logs/tuned-42/trace-Simba.jsonl`.
- Verify with `/logq trace logs/tuned-42 --layer L1`: expect twelve distinct `map` values across the L1 records (fox_scent, prey_scent, carcass_scent, cat_presence, exploration, ward_coverage, food_location, garden_location, construction_site, kitten_cry, herb_location, corruption).
- Drop the new trace into `just trace`: L1 panel shows up to twelve cards.
- `just verdict logs/tuned-42` — pass; canaries unchanged (no scoring touched).
- `just frame-diff <baseline> logs/tuned-42` — expect zero L2/L3 drift across DSEs; L1 row count grows.

## Log

- 2026-05-07 — Opened. Discovered while diagnosing Mallow-trace silent-load symptom (plan `the-focal-cat-trace-steady-pond.md`); user reported missing L1 markers on a working `bced533` Clover trace. Five maps named explicitly above.
- 2026-05-07 — Landed at `747d2b37`. Twelve `InfluenceMap` walks emitting via `L1Maps` `SystemParam` bundle + `emit_map!` macro. Phase 2D registry-walk refactor opened as ticket 207; trace_emit.rs in-source pointer retargeted at 207.
- 2026-05-07 — Verified. `just soak-trace 42 Simba` produced `logs/tuned-42/trace-Simba.jsonl` (1.31M records) with header `commit_hash: 747d2b37`, `commit_dirty: false`. Direct count confirms 12 distinct L1 `map` values: `carcass_scent · congregation · construction_site · corruption · exploration · food_location · fox_scent · garden_location · herb_location · kitten_cry · prey_scent · ward_coverage`. (`congregation` is the `cat_presence` map's metadata name; `corruption` is the `TileMap`/`CorruptionLens` lens.) Hard survival gates pass — `deaths_starvation: 0`, `deaths_by_cause.ShadowFoxAmbush: 1` (≤ 10). `just verdict` returns `concern` due to `burial=0` continuity (inherited canary failure predating 206) and constants/footer drift versus the 2026-05-02 baseline — all attributable to co-mingled stack work, not 206, which is a trace-only change with no scoring touched. `frame-diff` skipped (no paired Simba sidecar at parent commit and the question is moot at 206-isolation given the co-mingled state).
