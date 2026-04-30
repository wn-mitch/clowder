---
id: 052
title: §L2.10.7 plan-cost feedback substrate + cat target-taking roster
status: done
cluster: null
landed-at: 11f57d9
landed-on: 2026-04-28
---

# §L2.10.7 plan-cost feedback substrate + cat target-taking roster

**Landed:** 2026-04-28 | **Commits:** 11f57d9 (substrate + Hunt) · 1e5efe7 (Mate) · dbcb283 (Mentor) · 40a55b5 (ApplyRemedy) · 6322c9c (Socialize) · acccdc7 (GroomOther + Caretake + Fight + Build)

**Why:** §L2.10.7 of the substrate refactor (`docs/systems/ai-substrate-refactor.md` line 5535+) chose Mark ch 14's response-curve approach over a pathfinder-in-the-loop for plan-cost feedback. Until this ticket landed, every cat / fox DSE with a spatial input either binary-gated on range or ran a hand-rolled `(1 - dist/range).clamp(0,1)` scalar through the ScalarConsideration path — utility was blind to cost, and four §6.5 axes (`pursuit-cost`, `fertility-window` spatial, `apprentice-receptivity` pairing, `remedy-match` caretaker-distance) were explicitly blocked on the substrate.

**What landed:**

1. **Substrate** — `SpatialConsideration` enum variant in `src/ai/considerations.rs` carrying `LandmarkSource::{TargetPosition, Tile, Entity}` + a curve primitive. The substrate normalizes input as `cost = dist/range` so curve parameters land in `[0, 1]` units regardless of per-DSE candidate range. Wired through `evaluate_target_taking::score_target_consideration` so target-taking DSEs get per-candidate landmark resolution for free; `EvalCtx::entity_position` lookup added for `LandmarkSource::Entity` (production callers will land in successor work). The legacy unused influence-map sampling shape (every `sample_map` closure stubbed to 0.0, zero production callers) was retired in the same commit.

2. **Cat target-taking roster (scope item 2 — cat half)** — All 9 spatially-sensitive cat target-taking DSEs in `src/ai/dses/*_target.rs` now run through `SpatialConsideration`: Hunt, Mate, Mentor, ApplyRemedy, Socialize, GroomOther, Caretake, Fight, Build. Every `TARGET_NEARNESS_INPUT` const removed; every `pos_map` HashMap-lookup retired from the resolvers. Three idiom families emerged for closer-is-better axes:
   - **Logistic family (point-symmetric)** — `Logistic(s, m)` over `(1-cost)` ≡ `Composite{Logistic(s, 1-m), Invert}` over `cost`. Used by Mate, GroomOther, Fight. Behavior-neutral by construction.
   - **Quadratic family (non-symmetric)** — `Quadratic(exp=N, divisor=-1, shift=1)` over `cost` evaluates `(1 - cost)^N`, exactly preserving the legacy `nearness^N` shape. Used by Mentor, ApplyRemedy, Socialize, Caretake. The alternative `Composite{Quadratic, Invert}` would give `1 - cost^N` — different shape, doesn't match §L2.10.7's "Requires sustained proximity" rationale; the explicit-inversion idiom captures the right mathematical intent.
   - **Linear** — `Linear(slope=-1, intercept=1)` over `cost` evaluates `1 - cost` directly. Used by Build.

3. **Four §6.5 deferred axes (scope item 3)** — Hunt's `pursuit-cost` axis now uses spec'd `Logistic(steepness=10, midpoint=0.5, inverted)` over `range = HUNT_TARGET_RANGE`, retiring the `distance²` proxy. Mate's spatial half of `fertility-window`, Mentor's `apprentice-receptivity` spatial pairing, and ApplyRemedy's caretaker-distance variant all land via the `SpatialConsideration` substrate.

**Out of scope (deferred to successor):**

- **Cat self-state DSEs** (12 rows: Eat / Sleep / Forage / Explore / Flee / Patrol / Build / Farm / Herbcraft / PracticeMagic / Coordinate / Cook per §L2.10.7 line 5621+).
- **Fox dispositions** (9 rows per §L2.10.7 line 5648+).

These don't pass through `TargetTakingDse` and need their own substrate plumbing — including the first production callers of `LandmarkSource::Entity` (Den / Kitchen / Garden references) and a substrate decision on aggregate-centroid landmarks (Explore frontier, PracticeMagic corruption cluster, fox Hunting prey-belief centroid). Tracked at [065](../tickets/065-l2-10-7-self-state-fox-roster-sweep.md).

**Verdict — empirical balance hold.** Cumulative paired-baseline soaks across the 6 ports (seed 42, 15-min canonical) showed the colony-wide effect of the entire substrate refactor lands at essentially noise-level on every characteristic metric. From pre-052 (`1abaf49`) through bundled-port (`acccdc7`):

| Metric | pre-052 | through-Hunt | through-Mate | through-Mentor | through-ApplyRemedy | through-Socialize | through-bundled |
|---|---|---|---|---|---|---|---|
| Injury / Ambush / Starv | 1/4/1 | 1/4/1 | 1/4/1 | 1/4/1 | 1/4/1 | 1/4/1 | 1/4/1 |
| grooming | 264 | 269 | 262 | 262 | 262 | 268 | 262 |
| play | 834 | 842 | 834 | 834 | 834 | 842 | 834 |
| burial / courtship / mentoring / mythic-texture | 0/0/0/30 | 0/0/0/30 | 0/0/0/30 | 0/0/0/30 | 0/0/0/30 | 0/0/0/30 | 0/0/0/30 |
| never_fired_expected | 4 | 4 | 4 | 4 | 4 | 4 | 4 |
| Plan-failure total | (~284) | — | — | — | 284 | 300 | 280 |

Per-port f32 LSB churn (visible mainly through `TravelTo(SocialTarget): no reachable` argmax flips) ran in both directions across the 9 ports; the bundled GroomOther/Caretake/Fight/Build LSB drift happens to *cancel* Socialize's. Net change on every characteristic metric across the whole refactor: ≈ 0. **No drift exceeds the ±10% balance-methodology threshold; no hypothesis required.**

**Verification:** 1502 lib + integration tests pass. `just check` clean. `just verdict` survival canaries pass on every paired-baseline soak. Pre-existing canary collapses (Starvation > 0; courtship/mentoring/burial collapsed) were unchanged across all ports — ticket 040 (post-036 disposition shift), ticket 035 (burial not implemented), ticket 003 (mentor score magnitude) own those.

**Successor:** [065](../tickets/065-l2-10-7-self-state-fox-roster-sweep.md) — cat self-state DSEs + fox dispositions.

---
