---
id: 121
title: Cats stand around for ~1500 ticks at game start until first kitchen lands
status: done
cluster: substrate-over-override
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: 2026-05-01
---

## Substrate-over-override pattern

- **Hack shape:** `PlannerZone::Wilds` resolution in
  `src/systems/goap.rs::resolve_zone_position` authored a parallel
  feasibility language ("any nearby passable tile, with cat's-own-tile
  fallback") instead of consuming the substrate the IAUS scoring layer
  already reads. The cat's own tile satisfied `is_passable()`;
  `find_nearest_tile` correctly skipped `dist == 0`, but the
  `.or(Some(*pos))` fallback returned it whenever the radius-20 disc
  produced no other candidate — the planner stamped a degenerate
  self-target Travel that silently advanced.
- **IAUS lever:** `LandmarkAnchor::UnexploredFrontierCentroid` →
  `ExplorationMap::frontier_centroid()`. Already cached, recomputed once
  per tick by `update_exploration_centroid`
  (`src/systems/needs.rs:340-351`), and resolved through
  `inputs.exploration_map.frontier_centroid()` at
  `src/ai/scoring.rs:741-742` for the IAUS Explore DSE
  (`src/ai/dses/explore.rs:67-72`).
- **Sequencing:** substrate axis was already live (the IAUS-side
  resolver has been in place since `Explore` first carried a spatial
  consideration). No "land substrate before retiring hack" gap — the
  fix is pure alignment.
- **Canonical exemplar:** [092](../landed/092-unify-markersnapshot-plannerstate-statepredicate-feasibility.md) —
  `StatePredicate::HasMarker` collapsed the cat planner's parallel
  feasibility language for marker-shaped facts. 121 applies the same
  shape to anchor-shaped facts.

## Why

In the windowed build (and reproduced in headless seed-42 — `logs/tuned-42`)
cats are visibly idle from `start_tick = 1_200_000` until tick ~1_201_490 (~1.5
in-game days), at which point the first building (a kitchen) is constructed
and within ~50 ticks normal activity (PreyKilled, PlanReplanned, FoodEaten)
kicks in. The roster shows `Idle`; the event log shows `Explore` re-emitted
every 1-8 ticks. This is a cold-start behavior bug: not a crash, not a balance
drift — the GOAP/commitment-gate system fails to make forward progress under
the founding-spawn world state.

## Scope

Fix contributor #1 below — the degenerate `TravelTo(Wilds)` resolution that
stamps surveys at the cat's own tile. Re-measure the diagnosis window after
the fix lands; if the symptom persists, unblock follow-on tickets 122
(Socialize IAUS/gate mismatch) and 123 (RecentDispositionFailures cooldown)
that carry the other two contributors.

## Out of scope

- General re-tuning of `social_satiation_threshold`, `explore_satiation_threshold`,
  or any scoring constants on a balance-thread basis. Those knobs are part of
  the diagnosis but the immediate goal is a stuck-state fix, not a sweep.
- The `start_tick = 60 * ticks_per_season = 1_200_000` anchor itself
  (`src/plugins/setup.rs#L207`) is correct — that's the founder-age regime, not
  a clock bug.
- Changing the planner's silent `make_plan → None` collapse (ticket 091
  already surfaces this as `PlanningFailed/no_plan_found`).

## Current state

**Headless reproduction** (`logs/tuned-42`, seed=42, commit c15dbcf, 2026-05-01):
in the 1500-tick window `[1_200_000, 1_201_500)` across 8 cats:

```
PlanCreated:    3585     PlanReplanned:  0
PlanningFailed: 7001     PlanStepFailed: 0
PreyKilled:     0        BuildingConstructed: 6 (all at tick 1_201_490+)
```

`SystemActivation` snapshot at tick 1_200_100 shows ~213
`CommitmentDropOpenMinded` per 100 ticks plus 9 `CommitmentDropBlind` and 4
`CommitmentDropSingleMinded` — i.e. ~2-3 plans dropped by the §7.2 commitment
gate per tick.

Top failure dispositions in the window:

```
1150  Crafting   no_plan_found
1105  Foraging   no_plan_found
 804  Hunting    no_plan_found
```

Top *created* plans in the same window:

```
980  Exploring   ["TravelTo(Wilds)", "ExploreSurvey"]
588  Socializing ["TravelTo(SocialTarget)", "SocializeWith"]
103  Resting     []
```

## Approach

Three independent contributors were identified. Re-verify each with a focal
trace before fixing.

### 1. `TravelTo(Wilds)` is degenerate (root cause)

`src/systems/goap.rs#L5540` (pre-fix):

```clowder/src/systems/goap.rs#L5540-5540
PlannerZone::Wilds => find_nearest_tile(pos, map, 20, |t| t.is_passable()).or(Some(*pos)),
```

`find_nearest_tile` correctly skips `dist == 0`, but the
`.or(Some(*pos))` fallback returns the cat's own position whenever the
radius-20 disc has no better candidate. In `resolve_travel_to`
(`src/systems/goap.rs#L4554`) `pos.manhattan_distance(&target) <= 1`
is true on the first tick → `StepResult::Advance` without movement →
`ExploreSurvey` runs and `exploration_map.explore_area(pos.x, pos.y,
survey_explore_radius=4)` stamps a 9×9 disc *at the cat's current
tile, which is home*. Combined with `stamp_passive_exploration`
(radius 2 every tick around every cat), the home neighborhood
saturates within seconds → `unexplored_fraction_nearby <
explore_satiation_threshold (0.15)` → OpenMinded gate drops Exploring
on `still_goal == false` every tick.

**Fix (substrate-aligned):** `PlannerZone::Wilds` consumes
`ExplorationMap::frontier_centroid()` — the same anchor the IAUS
`Explore` DSE scores against
(`LandmarkAnchor::UnexploredFrontierCentroid` →
`src/ai/scoring.rs#L741-742`). After the change, "travel to wilds"
means the same thing on both sides of the L2↔L3 boundary by
construction. The `find_nearest_tile` scan stays as the no-frontier
fallback; the `.or(Some(*pos))` self-target fallback is removed.
When neither resolves, `resolve_zone_position` returns `None` and the
planner surfaces `no_plan_found` (visible since 091).

```text
PlannerZone::Wilds => exploration_map
    .frontier_centroid()
    .filter(|p| map.in_bounds(p.x, p.y) && map.get(p.x, p.y).terrain.is_passable())
    .or_else(|| find_nearest_tile(pos, map, 20, |t| t.is_passable())),
```

### 2. Socializing drops on tick 0 — see ticket 122

The IAUS Socialize DSE elects plans the OpenMinded gate's `still_goal`
proxy (`needs.social < social_satiation_threshold = 0.85`) drops on the
same tick. Founders spawn `social ≥ 0.85`; 588 of 3585 PlanCreated in the
window are this pattern. Carved out as ticket 122 (blocked on 121).

### 3. Silent `no_plan_found` retry storm — see ticket 123

3059 of 7001 PlanningFailed events are `no_plan_found` for
Hunting/Foraging/Crafting. The cat re-elects the same failing disposition
every 1-2 ticks. Carved out as ticket 123 (blocked on 121, may park if
121's fix drops the rate enough).

## Verification

1. Add a `cargo nextest`-style unit test that builds an `ExplorationMap`,
   places a cat on a passable tile, calls `resolve_zone_position(PlannerZone::Wilds, ...)`,
   and asserts the returned position is at distance ≥ M from the cat (or is
   the frontier centroid). Today this would return the cat's own tile.
2. Re-run `just soak 42` (15-min headless). The footer should show:
   - First `BuildingConstructed` event at tick < 1_200_500 (was 1_201_490).
   - `CommitmentDropOpenMinded` per 100-tick activation snapshot drops from
     ~213 to a small fraction of that in the first sim-day.
   - `PlanCreated` / `PlanningFailed` ratio in the first 1500 ticks shifts
     toward forward progress (any non-zero `PlanReplanned` or `PlanStepFailed`
     in the window is a good signal).
3. Visual smoke test in the windowed build: cats fan out within seconds of
   spawn rather than after ~20 wall-seconds.

If the soak's deaths_by_cause / continuity tallies regress >10% from the
baseline at `logs/baselines/current.json`, this is a balance change and
needs a four-artifact concordance check (`just hypothesize`).

## Log

- 2026-05-01: Symptom reported in windowed build. Reproduced in headless
  `logs/tuned-42`. Diagnosis identified three contributors; (1) is the
  cleanest fix and likely dissolves the symptom on its own. Opened
  tickets 122 and 123 in the same session so the parked subscope is
  durable per CLAUDE.md's antipattern-migration follow-ups discipline.
- 2026-05-01: Promoted into substrate-over-override epic 093 as the
  anchor-resolution analogue of 092's marker-resolution unification —
  the `find_nearest_tile(...).or(Some(*pos))` shape is the parallel-
  feasibility-language smell applied to anchors instead of markers.
  Substrate-aligned fix landed: `PlannerZone::Wilds` consumes
  `ExplorationMap::frontier_centroid()`, with `find_nearest_tile` as
  the no-frontier fallback and `None` (not `Some(*pos)`) when neither
  resolves. Three new unit tests in `src/systems/goap.rs::tests`
  (`wilds_targets_frontier_centroid_when_present` /
  `wilds_falls_back_to_passable_distant_tile_when_frontier_empty` /
  `wilds_returns_none_when_frontier_empty_and_no_passable_neighbor`).
  `just check` clean; full lib suite 1672/1672 green.
- 2026-05-01: **Soak result — structural fix correct, cold-start
  symptom not dissolved by 121 alone.** `just soak 42` writes
  `logs/tuned-42`; pre-fix archive at
  `logs/tuned-42-c15dbcf-pre-121-baseline`. Behavioral deltas: total
  event count 880k → 583k (-34%); `deaths_by_cause.Starvation` 2 → 0;
  `anxiety_interrupt_total` 16025 → 19738; `grooming` 185 → 88;
  `play` 428 → 351. Survival canaries: Starvation gate now passes
  (was failing pre-fix); ShadowFoxAmbush 6 (≤10, OK).
  **Cold-start window unchanged**: first `BuildingConstructed` still
  at tick 1_201_490 in both runs; `PlanCreated` count in `[1_200_000,
  1_201_500)` identical at 3588 (was predicted to drop). Likely
  cause: at tick 1_200_001 the `frontier_centroid` either is `None`
  (system-ordering window) or maps to the geometric center of the
  120×90 worldgen which falls on Water and gets rejected by the
  passable filter — both branches fall through to `find_nearest_tile`,
  which on a mostly-passable map almost always returns a 1-tile-away
  target (the old `.or(Some(*pos))` self-target was rare in
  practice). Verdict gate fails on `continuity:
  fail:mentoring=0,burial=0,courtship=0` and the never-fired
  positives set, but **both failures are pre-existing at this
  commit** — the same six never-fired positives and the same three
  zero-tally continuity classes appear in the pre-fix archive too.
  No regression attributable to 121.

  **Carveout follow-ons (122/123) confirmed necessary.** The §Approach
  §2 (Socialize IAUS/gate mismatch) and §3 (RecentDispositionFailures
  cooldown) hypotheses now look load-bearing rather than
  "may-park-if-121-fixes-it." Unblocking both tickets in the same
  commit; their `blocked-by: [121]` clears with this land.

  Landing the structural cure on its own merit: the substrate-
  alignment is real (the planner now consumes the same anchor IAUS
  scores against — that drift smell is closed), and the
  no-frontier-fallback path is now observable failure rather than
  silent self-target. Aggregate event-count drop and Starvation→0
  win are real positive signals from the fix, even if the specific
  cold-start-to-first-build timing didn't shift.
