---
id: 001
title: Explore DSE rebalance — Sub-2 saturation curve
status: done
cluster: null
landed-at: null
landed-on: 2026-04-24
---

# Explore DSE rebalance — Sub-2 saturation curve

**What shipped:**

- Survey stamps a visible disc (radius 4 = 9×9 = 81 tiles) instead of a
  single tile. New `ExplorationMap::explore_area()` method; new
  `survey_explore_radius` constant in `DispositionConstants`.
- Explore DSE's `unexplored_nearby` axis: identity `Linear(1, 0)` →
  `Logistic(10, 0.3)` ("explore_saturation"). Sharp decay when ≥70% of
  local tiles are explored. New named curve in `curves.rs`.
- Explore DSE's `curiosity` axis: `Linear(1, 0)` → `Linear(0.7, 0)`,
  wiring in the previously-unused `explore_curiosity_scale` constant.
- `ExploreDse::new()` now accepts `&ScoringConstants`; 4 registration
  sites updated.

**Hypothesis:** Replacing identity curves with a Logistic saturation curve
on `unexplored_nearby` and scaling curiosity by 0.7, combined with survey
stamping a visible disc instead of a point, breaks the Explore dominance
loop. The fraction of unexplored tiles will actually drop as cats survey,
and the Logistic curve amplifies the suppression past 70% explored.

**Predictions:** Explore action-time fraction ↓ substantially (from ~45%);
Wander / Socialize / Groom ↑; survival canaries flat.

**Soak deferred** — bundled into a larger soak with other in-flight work.

**Files:** `src/ai/dses/explore.rs`, `src/ai/curves.rs`,
`src/resources/exploration_map.rs`, `src/steps/disposition/survey.rs`,
`src/resources/sim_constants.rs`, `src/plugins/simulation.rs`,
`src/main.rs`, `src/ai/scoring.rs`.

---

## §4 capability markers batch 2 — CanHunt / CanForage / CanWard / CanCook

**Landed:** 2026-04-24 | **Ticket:** 014

**What shipped:**

New `src/ai/capabilities.rs` — single `update_capability_markers` system
authoring 4 per-cat capability markers with spec-intent life-stage rules:

| Marker | Predicate |
|--------|-----------|
| `CanHunt` | (Adult ∨ Young) ∧ ¬Injured ∧ ¬InCombat ∧ forest nearby |
| `CanForage` | ¬Kitten ∧ ¬Injured ∧ forageable terrain nearby |
| `CanWard` | Adult ∧ ¬Injured ∧ HasWardHerbs |
| `CanCook` | Adult ∧ ¬Injured |

KEY constants added to `CanForage`, `CanWard`, `CanCook` in `markers.rs`.
DSE `.require()` cutover: HuntDse, ForageDse, HerbcraftWardDse, CookDse.
Retired `can_hunt`, `can_forage`, `has_ward_herbs` from `ScoringContext`.
Removed inline `if ctx.can_hunt` / `if ctx.can_forage` / `let ward_eligible`
gates in `scoring.rs`. Removed `has_nearby_tile` + `find_nearest_tile` from
`disposition.rs` (dead after cutover). MarkerSnapshot population extended
in both `goap.rs` and `disposition.rs`. System registered in Chain 2a
(simulation plugin + build_schedule). 23 new tests.

**Life-stage design decision:** Young cats hunt (badly — skill gates outcome
quality, not the capability marker) and forage. Elders forage but don't hunt
(reduced physical capacity). Kittens excluded from all. CanCook is purely
per-cat (Adult ∧ ¬Injured); colony-scoped kitchen/food markers stay on
CookDse to preserve the `wants_cook_but_no_kitchen` build-pressure signal.

**Hypothesis:** Elder cats excluded from hunting → Hunt feature count drops
~5–15%. Young cats still hunt (just worse). Forage minimal change. Ward/Cook
minimal change (already effectively Adult-gated by skill thresholds).

**Soak (seed 42, `--duration 900`):** Starvation 0, ShadowFoxAmbush 0,
0 deaths total, footer written. Colony survived the full 15-min soak.
Never-fired-expected (10) and continuity gaps are pre-existing (ticket 014
deferred). No regression from capability markers.

**Files:** `src/ai/capabilities.rs` (new), `src/components/markers.rs`,
`src/ai/mod.rs`, `src/ai/scoring.rs`, `src/ai/dses/hunt.rs`,
`src/ai/dses/forage.rs`, `src/ai/dses/herbcraft_ward.rs`,
`src/ai/dses/cook.rs`, `src/systems/goap.rs`, `src/systems/disposition.rs`,
`src/plugins/simulation.rs`, `src/main.rs`.

---
