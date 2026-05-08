---
id: 228
title: cat route-cost field as L1 perception + Field Consideration variant
status: in-progress
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

223's soak surfaced a regression that 223's path-cost overlay alone
can't close. With the legacy `FoxTerritorySuppression` damp branch
retired, cats land in fox-scent corridors at *decision time*
(Hunt/Forage/Patrol/Wander/Explore scores no longer suppressed in fox
territory) and then thrash when `acute_health_adrenaline_flee` preempts
every plan tick after their hunger crashes. Three adult Starvation
deaths clustered at adjacent tiles in the [33,21]–[31,22] fox corridor;
0 courtship events; `MatingOccurred` / `CourtshipInteraction` /
`PairingIntentionEmitted` never fired.

**The substrate-grade fix is a chained influence map**, not a
modifier and not an L2 axis on the cat-position scalar. Existing
substrate already encodes "where danger lies" (`FoxScentMap`,
`CorruptionLens`). The missing piece is a *cat-centric, destination-aware*
perception of "how costly is reaching that tile, given the dangers
between me and it" — a per-cat scalar field where each tile carries
cost-to-reach-from-cat under overlay-aware edge weights. That field
*is* L1 perception. The L2 read site samples it at a candidate
destination via a new `Consideration::Field` variant.

This is the canonical pattern from the influence-map / Dijkstra-map
literature: **Brian Walker's "Incredible Power of Dijkstra Maps"
(Brogue, 2010)** — a scalar field of cost-to-reach, flooded once with
arbitrary edge weights, queried by N agents; **Mark/Dill influence-
map composition (GDC AI Summit)** — chain output of one map into
another, score utility over reads. **Khatib 1986 potential fields**
gave the academic origin. The user's framing in the post-cluster
reflection mapped directly onto this lineage.

The §4.7 substrate-vs-search-state boundary applies cleanly:
- **L1 today**: world-centric InfluenceMaps + cat-centric scalars
  (`escape_viability` at `src/systems/interoception.rs:222`,
  `fox_scent_level` at cat position).
- **L1 addition (this ticket)**: cat-centric **route-cost field** —
  per-cat, per-replan Dijkstra flood from the cat outward. Edge
  weights = `terrain.movement_cost() + Σ(weighted overlays)` (the
  same inputs 222/223/224 already feed into A*). Result: for any
  tile T, `route_cost[T]` = total cost-to-reach including fox-scent,
  corruption, and the cat's own boldness factor.
- **L2 read site**: a new `Consideration::Field(FieldConsideration)`
  variant samples `route_cost[D]` at a candidate destination D.
  Hunt's TargetPosition axis becomes "route-cost to candidate prey,"
  not "Manhattan distance." Patrol's TerritoryPerimeterAnchor axis
  becomes "route-cost to perimeter tile." High route-cost → low
  score axis → CP gate suppresses the disposition in proportion to
  the *real* cost (terrain + danger + per-cat caution) of reaching
  that destination. This is the destination-aware refinement that
  `src/ai/dses/patrol.rs:108` already names ("reserved for a
  destination-aware refinement once the SpatialConsideration variant
  lands").
- **L3**: unchanged.
- **Plan execution**: walk the gradient of the route-cost field from
  cat to chosen destination. Standard Brogue pattern. Cat-side A*
  becomes redundant (foxes still use A* — they don't carry route-cost
  fields).

This reframe is more substrate-aligned than the previous v2 framing
(extend 209's L2 cost-axis pattern from Patrol-only to the four
siblings, reading cat-position `fox_scent_level`). The cat-position
scalar suppresses score even when the cat sits *near* a fox corridor
at the colony hub; the destination-aware route-cost suppresses score
when the *path to the chosen target* is risky — which is what we
actually want at decision time.

## Scope

- **New module** `src/ai/route_cost.rs` (or
  `src/components/route_cost_field.rs` — pick during impl):
  - `pub struct RouteCostField { costs: Vec<u32>, width: u32, height: u32 }`
    (flat row-major grid; `cost_at(pos: Position) -> u32` accessor).
  - `MAX_COST_BUDGET` sentinel for unreachable tiles (or pick
    `u32::MAX`).
  - Cached on the cat as a `Component` (`HasRouteCostField` /
    similar), populated lazily on replan, invalidated on cat
    movement past a threshold or on overlay changes.

- **Flood-fill helper** `src/ai/pathfinding.rs::flood_dijkstra`:
  - Signature: `fn flood_dijkstra(from: Position, map: &TileMap, overlays: &[WeightedOverlay<'_>], max_cost: u32) -> RouteCostField`.
  - **Use flat-queue / bucketed Dijkstra**, not `BinaryHeap`. Edge
    costs here are small integers (terrain 1-4 + overlay 0-18
    after per-cat boldness weighting = 1-22). Walker's blog +
    standard literature: bucketed Dijkstra is order-of-magnitude
    faster than binary-heap when costs fit a small bucket count.
    Build the helper anew rather than reusing the existing A*
    `BinaryHeap` at `pathfinding.rs:229`.
  - Cost-cap budget bounds the flood radius so far-away cats don't
    traverse the full map (Walker's standard optimization).

- **New Consideration variant**
  `Consideration::Field(FieldConsideration)`:
  - At `src/ai/considerations.rs:312` (the `enum Consideration`
    union). Variants today: `Scalar / Spatial / Marker`.
  - `FieldConsideration { name, source: FieldSource, landmark: LandmarkSource, range: f32, curve: Curve }`.
    Mirrors `SpatialConsideration`'s shape but reads field-cost at
    the landmark instead of computing Manhattan distance.
  - `FieldSource` enum: initial variant `OwnRouteCost`. Future:
    other cat-centric or world-centric fields (heat map, scent
    gradient, etc.).
  - The evaluator's per-variant dispatch resolves the cat's
    `RouteCostField`, looks up `cost_at(landmark_position)`,
    normalizes by `range`, runs through `curve`. Returns 0.0
    if the landmark is unreachable (`MAX_COST_BUDGET` or beyond
    the flood radius) — same closer-is-better convention as
    SpatialConsideration.

- **DSE wiring** — replace the cat-position
  `ScalarConsideration("fox_scent_level", Composite{Logistic, Invert})`
  in Patrol (`src/ai/dses/patrol.rs:98–123`) with
  `Consideration::Field(FieldConsideration { source: OwnRouteCost,
  landmark: TerritoryPerimeterAnchor, ... })`. Add the equivalent
  axis to **Hunt**, **Forage**, **Wander**, **Explore**, each at the
  appropriate landmark (TargetPosition for target-taking DSEs;
  per-DSE Anchor variants for self-state DSEs).

- **Constants in `ScoringConstants`** (`src/resources/sim_constants.rs`):
  - Replace `patrol_fox_scent_weight` with `patrol_route_cost_weight`
    (or keep the old name and re-purpose the read).
  - Add `hunt_route_cost_weight`, `forage_route_cost_weight`,
    `wander_route_cost_weight`, `explore_route_cost_weight`.
  - All ship dormant at 0.0; tuning is a separate follow-on.

- **Plan execution** — gradient-descent path extraction from
  `RouteCostField` replaces cat-side A* `find_path` calls. From the
  cat's position, walk to the lowest-cost neighbor each tick until
  reaching the destination. Foxes keep A* (they don't carry
  route-cost fields).

- **Boldness conditioning** — moves from per-call `WeightedOverlay`
  (224) to flood-time edge weights. The cat's own boldness
  determines the overlay weight when its flood is built.
  `WeightedOverlay` may be partially or fully retired for cat-side
  use; foxes keep it (they still call A* with overlays).

- **Tests**:
  - Unit: `flood_dijkstra` correctness on a synthetic 5×5 grid with
    one high-cost overlay tile blocking the direct route — assert
    costs propagate around it and the gradient yields a detour.
  - Unit: gradient-descent path from `RouteCostField` matches A*
    output on no-overlay terrain (fallback equivalence).
  - Unit: `FieldConsideration` returns expected score on a
    fixture `RouteCostField` for both reachable and unreachable
    landmarks.
  - Per-DSE `dormant` / `active` tests parallel to
    `patrol.rs::tests` — dormant default leaves CP unchanged;
    active weight adds the new variant with the correct curve and
    weight.
  - Microexperiment via `just scenario`: cat at (0,0), prey at
    (10,0), fox-scent corridor at x=5. Bold cat picks (10,0); timid
    cat picks alternate prey OR alternate disposition. Assert L2
    score difference matches the flood-cost difference.

## Out of scope (open as separate follow-ons if needed)

- **Reverse-from-shared-destination caching** (e.g., "everyone wants
  the food store"). Profile first; don't over-engineer. Forward-
  from-cat per replan is the default per Brogue's pattern.
- **Generalizing `InfluenceMap` trait** to take an entity context.
  Keep the new field as a Component / cat-keyed Resource for now.
  Cat-centric perception lives outside the world-keyed registry.
- **Full retirement of fox-side A***. Foxes keep A* until/unless
  they need substrate paths. Cat-side A* may retire, partially or
  fully — pick during impl.
- **Tuning the `*_route_cost_weight` constants non-zero**. Ships
  dormant; tuning is the next ticket after substrate stabilizes.
  Parallel to how 209 → 211 worked.
- **Re-adding `FoxTerritoryHuntSuppression` as a `ScoreModifier`**.
  Explicitly the wrong layer — that was the v1 reframe rejected by
  the substrate-over-override discipline.
- **Extending 209's cat-position L2 axis to four sibling DSEs**.
  That was the v2 reframe (cat-position `fox_scent_level` on
  Hunt/Forage/Wander/Explore). Subsumed by this v3 destination-
  aware shape.

## Current state

- 209 wired `patrol_fox_scent_weight` (Patrol L2 axis, dormant at
  0.0). Reference impl: `src/ai/dses/patrol.rs:98–123`.
- 222 landed `TileCostOverlay` substrate.
- 223 landed cat-side path-cost overlays (`FoxScentOverlay`,
  `CorruptionOverlay`) and retired the legacy
  `FoxTerritorySuppression` modifier (renamed → `FleeFoxScentBoost`,
  Flee additive branch only). 223's soak surfaced the
  decision-time gap (3 adult Starvation, 0 courtship,
  never-fired Mating in the [33,21]–[31,22] fox corridor).
- 224 landed boldness-conditioned `WeightedOverlay` weights on the
  path-cost layer. This ticket may partially retire that API for
  cat-side use (boldness moves to flood-time edge weights instead).
- After this ticket lands and weights are tuned, the cluster's
  intent is fully expressed: cats avoid choosing destinations behind
  costly (terrain + danger) routes, AND the gradient-descent walk
  to the chosen destination naturally routes around fox scent. One
  substrate object (the route-cost field) carries both signals.

## Approach

1. **Build `RouteCostField` + `flood_dijkstra` first.** Pure substrate
   + helper, no DSE wiring. Unit-test the flood and the gradient
   descent in isolation. This is the load-bearing piece — get it
   right before threading DSE reads.
2. **Add `Consideration::Field(FieldConsideration)`.** Extend the
   evaluator's per-variant dispatch in
   `src/ai/eval.rs` (or wherever the DSE evaluator lives — verify
   during impl). Returns 0.0 for unreachable / out-of-budget
   landmarks. Mirror `SpatialConsideration::evaluate` shape.
3. **Wire one DSE first — Patrol** (since 209 already has the
   Pattern). Replace the existing `ScalarConsideration("fox_scent_level", ...)`
   with `FieldConsideration { source: OwnRouteCost, landmark:
   TerritoryPerimeterAnchor, ... }`. Update `patrol_route_cost_weight`
   constant. Verify tests pass at dormant; dormant default produces
   CP-unchanged.
4. **Extend to Hunt / Forage / Wander / Explore.** One DSE at a
   time, each as a clean diff. Ship dormant at 0.0 for each.
5. **Cat-side `find_path` retirement** (or fallback wiring). Each
   call site at the post-224 state currently constructs
   `WeightedOverlay` slices and calls `find_path`. Replace with a
   `RouteCostField` lookup + gradient-descent step. Foxes' call
   sites untouched. This step is the largest diff; sequence after
   the L2 wiring is solid.
6. **Boldness conditioning at flood time.** The cat's flood reads
   its own `personality.boldness` and weights overlays accordingly.
   `WeightedOverlay` may be retired on the cat side after this step.
7. **Doc comments** — code comments at `RouteCostField` definition,
   `flood_dijkstra` doc, `FieldConsideration` rustdoc, and each DSE
   read site. Frame the substrate-vs-search-state boundary
   explicitly: route-cost field is *substrate* (cat perceives
   "how hard is reaching there" the same way it perceives "how
   bright is the sun"); not search state.

## Verification

- `just check && just test` — substrate ships dormant; existing
  tests pass; new flood-fill + Field Consideration unit tests pass;
  per-DSE dormant/active tests pass.
- `just scenario` — microexperiment: cat at (0,0), prey at
  (10,0), fox-scent corridor at x=5. Bold cat picks (10,0); timid
  cat picks alternate prey OR alternate disposition.
- No soak required for the dormant ship — the L2 score paths are
  unchanged at `weight = 0.0` and the gradient-descent path
  matches A* at zero overlays. Tuning ticket follow-on does the
  soak when it lifts the weights.
- After tuning lifts at least one weight non-zero, soak predictions
  match the 223 regression analysis:
  - Adult Starvation drops to 0 (cats avoid hunting in fox
    corridors at decision time per the route cost).
  - Courtship returns non-zero; `MatingOccurred` fires.
  - ShadowFoxAmbush trends ≤ post-cluster baseline.
  - `just frame-diff` between post-cluster and post-tuning shows
    Hunt / Forage scores DROP for prey/forage targets behind
    fox-scent corridors; rise for safer-route targets.
- Drift > ±10% triggers four-artifact hypothesis at
  `docs/balance/<N>-route-cost-substrate.md` (in the tuning
  follow-on, not here).

## Risks / open questions

- **Performance**: per-cat per-replan flood at 100×100 grid +
  flat-queue Dijkstra. Microbenchmark during impl. Cost-cap
  optimization (Walker) bounds the flood radius. Cache + invalidate
  on overlay changes (corruption decay, fox-scent decay) is the
  fallback if profiling shows hotspot.
- **Cat-keyed substrate** is a new abstraction. Possible step toward
  generalizing `InfluenceMap`; deferred for now. Risk: divergent
  shape (cat-keyed perception + world-keyed perception) confuses
  future readers. Mitigation: code comment + Log line explicitly
  framing this as "cat-centric physics perception, same family as
  `escape_viability`."
- **A\* retirement on cat side**. Carefully scoped. Foxes need A\*;
  cats might too as a fallback if the field is stale. Decision
  during impl: full retirement vs gradient-descent-with-A\*-fallback.
- **`WeightedOverlay` collapse**. 224's API may be partially or
  fully retired for cat-side use. Touches every cat-side
  `find_path` call site (~10 sites). Plan the collapse as part of
  this ticket's impl, not as a separate follow-on.

## Log

- 2026-05-07: opened from 223's verification regression — soak showed
  3 adult Starvation deaths clustered in the [33,21]–[31,22] fox
  corridor + 0 courtship + never-fired Mating canaries. Theory of
  the case: 223's path-cost overlay is route-time substrate; the
  retired damp branch was decision-time substrate. They compose
  orthogonally; 223 alone leaves a gap at the decision layer.
- 2026-05-07: reframed v1 → v2 via substrate-over-modifier discussion.
  Original draft proposed re-adding `FoxTerritoryHuntSuppression`
  as a `ScoreModifier`; that's exactly the override pattern the
  substrate refactor is retiring. Reframed scope to extend 209's L2
  cost-axis pattern from Patrol to its four siblings
  (Hunt/Forage/Wander/Explore), shipping dormant at 0.0 with tuning
  as a separate follow-on.
- 2026-05-07: reframed v2 → v3 via post-cluster architectural
  reflection. The v2 cat-position scalar shape (read `fox_scent_level`
  at the cat) is a stopgap — destination-aware route-cost is the
  proper substrate per §4.7 and the Walker / Mark-Dill / Khatib
  literature. v3 introduces a chained influence map: cat-centric
  Dijkstra cost-field as L1 perception, sampled at candidate
  destinations via a new `Consideration::Field` variant. Subsumes
  the v2 scope — `patrol.rs:108`'s "reserved for a destination-aware
  refinement once the SpatialConsideration variant lands" comment
  was already gesturing at this exact shape. Implementation
  collapses 222/223/224's path-cost overlay into the same substrate
  object; cat-side A\* may retire in favor of gradient-descent path
  extraction from the field. Foxes keep A\*. WeightedOverlay
  partially retired on cat side.
