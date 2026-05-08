---
id: 223
title: cat paths respect fox-scent + corruption
status: ready
cluster: pathfinder-risk-awareness
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why
With ticket 222's `TileCostOverlay` substrate in place, wire cat-side
A* to read `FoxScentMap` and `CorruptionLens` as routing costs. This
is the work that closes ticket 214's prediction structurally — cats
route around fox territory because the routing math says it is
cheaper, not because a DSE score got damped *after* the route was
already chosen.

This ticket also retires the damping branch of
`FoxTerritorySuppression` (`src/ai/modifier.rs:802-869`) for cat-side
DSEs. That modifier was the post-CP "you are in fox territory; lower
your Patrol/Hunt/Forage/Wander score" damp. Once paths route around
fox territory, that signal is double-priced: the cat is no longer
*in* fox territory at decision time, and if it is, the path it picks
to leave will already cost more. Keep the modifier's Flee-additive-
boost branch — that is a score boost, not a movement cost, and
doesn't fold into path-cost.

## Scope
- `impl TileCostOverlay for FoxScentMap` reading
  `FoxScentMap::base_sample(pos)` and converting [0,1] → u32 cost.
- `impl TileCostOverlay for CorruptionLens` reading the same way over
  the corruption field.
- Wire cat-side `find_path` / `step_toward` call sites with the
  overlay set `[&fox_scent_map, &corruption_lens]`. Fox-side sites
  (`src/steps/fox/mod.rs`) keep `&[]` — foxes do not avoid their own
  scent.
- Split `FoxTerritorySuppression` into:
  - `FleeFoxScentBoost` (keeps the additive Flee-boost branch)
  - delete the Hunt/Forage/Patrol/Wander/Explore damping branch
- Update influence-map registry tests if any reference the old
  modifier's output shape.
- Update / re-anchor any test that asserted post-modifier Patrol score
  damping at high fox scent — those become detour-cost assertions in
  pathfinding tests, or are deleted if redundant.

## Out of scope
- Personality (boldness) conditioning of overlay weights — ticket 224.
- Fox-side overlay sets (e.g., wards repel foxes, prey-scent attracts
  them) — separate follow-on if needed.
- `KillByPredatorScentMap` discrimination from `CarcassScentMap` —
  209 already names this as an unrelated follow-on.
- Building / non-cat species pathfinding overlays — this ticket is
  cat-only by deliberate scope.

## Current state
- Ticket 222 lands the substrate (trait + API change + empty-slice
  call sites). This ticket lands behavior change against that
  substrate.
- Influence maps `FoxScentMap` and `CorruptionLens` are already
  registered in `populate_influence_map_registry` at
  `src/plugins/simulation.rs:131-160` (per CLAUDE.md "InfluenceMap
  registry stubs are forbidden" — they have readers + writers).
- Cluster A→B→C; B is this ticket; C wires personality on top.

## Approach
1. **`impl TileCostOverlay for FoxScentMap`.**
   Scaling math: `terrain.movement_cost()` for Grass=1, Forest=3.
   A 5-tile detour to skirt a single fox-scent tile costs at most
   4 extra grass tiles = 4 cost; a sharper detour (across forest)
   costs more. So fox-scent at max should add **≥4** to a single
   tile's cost to make the detour preferred. Suggest:
   ```rust
   ((scent.clamp(0.0, 1.0) * 8.0) as u32)
   ```
   Max contribution 8 (≥4 buffer for forest detours; ≤8 keeps the
   bound sane on long paths). Document the rationale in the impl;
   exposes a constant in `ScoringConstants` if balance-tuning surfaces
   the need.

2. **`impl TileCostOverlay for CorruptionLens`.**
   Symmetric shape; tile corruption [0,1] → u32 with similar
   magnitude. Tune the constant alongside fox-scent's so the
   relative weight reflects ecological intent (corruption is
   typically rarer and more corrosive — slightly higher max cost
   may be warranted; verify in soak).

3. **Wire cat-side call sites.** Build a helper:
   ```rust
   fn cat_path_overlays<'a>(
       fox: &'a FoxScentMap,
       corr: &'a CorruptionLens,
   ) -> [&'a dyn TileCostOverlay; 2] {
       [fox, corr]
   }
   ```
   Each call site borrows the maps via `Res<FoxScentMap>` /
   `Res<CorruptionLens>` (or however the existing system params
   read them) and calls the helper. Fox-side sites (`steps/fox/`)
   keep `&[]`.

4. **Retire the damping branch of `FoxTerritorySuppression`.**
   Refactor:
   - New modifier `FleeFoxScentBoost` (keeps `is_flee` branch + the
     `flee_boost_scale = 0.5` constant).
   - Delete `is_damped` branch (Hunt/Forage/Patrol/Wander/Explore).
   - Update `populate_modifier_registry` (or equivalent) to register
     the new modifier under the same hook.
   - Walk the test file; tests that asserted damping behavior
     re-anchor on path-detour cost in `pathfinding.rs` tests.

5. **§4.7 substrate-vs-search-state classification.**
   Path-cost overlays are *substrate*: the cat senses scent in the
   environment it moves through. They are not search state. Document
   this in this ticket's Approach so future readers do not
   misclassify per the §4.7 boundary that ticket 092 misclassified
   (per CLAUDE.md "AI substrate refactor" section).

## Verification
- `just check && just test` — all unit tests pass.
- `just soak-trace 42 Wren` + `just verdict logs/tuned-42` +
  `just frame-diff logs/tuned-42-post-209 logs/tuned-42`.
- **Predictions:**
  - `ShadowFoxAmbush` deaths drop versus post-209 baseline of 3.
  - Patrol / Hunt / Forage path lengths increase modestly in
    fox-heavy zones (visible as `position_dwell` per cat in trace,
    or via summary-level `tiles_walked_per_action` if available).
  - Survival gates pass: Starvation 0; ShadowFoxAmbush ≤ 10;
    footer line written; never-fired-expected-positives 0.
  - All six continuity canaries non-zero (grooming, mentoring,
    play, burial, courtship, mythic-texture).
  - Wren focal trace shows `find_path` returning detour routes when
    fox-scent is high near the cat-to-target geodesic.
- Drift > ±10% on any characteristic metric (action shares,
  population trajectories, mood valence) requires a four-artifact
  hypothesis at `docs/balance/<N>-pathfinder-fox-scent.md` per
  CLAUDE.md balance discipline.
- A refactor that changes sim behavior is a balance change — soak
  before landing.

## Log
- 2026-05-07: opened from work-214 investigation. Blocked-by 222.
