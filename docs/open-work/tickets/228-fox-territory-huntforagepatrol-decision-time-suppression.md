---
id: 228
title: fox-territory hunt/forage/patrol decision-time suppression
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

223's soak surfaced a regression that 223's path-cost overlay alone
can't close. With the legacy `FoxTerritorySuppression` damp branch
retired, cats land in fox-scent corridors at *decision time*
(Hunt/Forage/Patrol/Wander/Explore scores no longer suppressed in fox
territory) and then thrash when `acute_health_adrenaline_flee` preempts
every plan tick after their hunger crashes. Three adult Starvation
deaths clustered at adjacent tiles in the [33,21]–[31,22] fox corridor
in the verification soak; 0 courtship events; `MatingOccurred` /
`CourtshipInteraction` / `PairingIntentionEmitted` never fired.

The path-cost overlay (223) is *route-time* substrate — it gates which
A* path the cat takes *given* a destination. The retired damp branch
was *decision-time* gating — it suppressed the L2 score that says "I
want to Hunt here." These compose orthogonally per §4.7
substrate-vs-search-state — both are substrate, but they answer
different questions:

- **Score-time gate** ("should I do this *here*?") — at high local fox
  scent, the L2 score for risky-territory dispositions tilts the L3
  softmax toward Eat / StockpileForage / Move-out-of-area.
- **Route-time gate** ("which route do I take?") — path-cost overlay
  raises A* edge cost on fox-scent tiles so paths to non-fox
  destinations route around the corridor. (Landed 223.)

**Substrate over modifier.** The naive shape — re-add a
`FoxTerritoryHuntSuppression` `ScoreModifier` that multiplicatively
damps Hunt/Forage/Patrol/Wander/Explore — would re-introduce the exact
override pattern the substrate refactor is retiring. Ticket 209 already
lit the right shape: it wired `patrol_fox_scent_weight` into Patrol's
CompensatedProduct as an L2 cost-axis reading `fox_scent_level` with
`Composite{Logistic(6.0, 0.4), Invert}`. Ships dormant at 0.0; the
score reflects "how risky is here" at decision time *naturally*, no
post-CP modifier kicking in. This ticket extends that pattern from
Patrol-only to its four DSE siblings.

## Scope

- Extend ticket 209's L2 cost-axis pattern (`src/ai/dses/patrol.rs:98–123`)
  to **Hunt**, **Forage**, **Wander**, and **Explore**:
  - Each DSE's CompensatedProduct gains a conditionally-added
    `ScalarConsideration` reading `fox_scent_level` with the same
    `Composite{Logistic(6.0, 0.4), Invert}` curve.
  - Conditional add gate matches Patrol's: the axis is only present
    when its weight `> 0.0` (CP semantics `c·0 = 0` would zero the
    product if a 0-weight axis were always present).
- New per-DSE constants in `ScoringConstants` (`src/resources/sim_constants.rs`),
  paralleling `patrol_fox_scent_weight`:
  - `hunt_fox_scent_weight: f32`
  - `forage_fox_scent_weight: f32`
  - `wander_fox_scent_weight: f32`
  - `explore_fox_scent_weight: f32`
  - All ship dormant at `0.0` with a `default_*_fox_scent_weight()`
    function each. Tuning is a follow-on ticket per the 209 precedent.
- Per-DSE unit tests mirroring patrol.rs's `dormant`/`active`
  pattern: dormant default leaves CP unchanged; active weight adds
  the consideration with the correct curve and weight.

## Out of scope

- **Re-adding `FoxTerritoryHuntSuppression` as a `ScoreModifier`.**
  Explicitly the wrong layer per the substrate-over-override
  discipline; this ticket exists to do the substrate-side fix
  instead.
- **Tuning the new weights to non-zero values.** Ships dormant; a
  follow-on tuning ticket (analog of how 209 was opened then 211
  tuned the Coordinate weight) lifts the weights once the substrate
  is in place.
- **Destination-aware fox-scent axis.** Patrol's existing axis reads
  `fox_scent_level` (cat-position scalar). The line at
  `patrol.rs:108` notes "this axis is reserved for a destination-aware
  refinement once the SpatialConsideration variant lands." This
  ticket matches the current cat-position shape; the destination-aware
  lift is a separate follow-on after the SpatialConsideration variant
  exists.
- **Boldness conditioning of the L2 axis.** 224 conditions the
  *path-weight* on boldness; conditioning the *L2 axis weight* on
  boldness is a parallel-shape follow-on if balance work surfaces
  the need (likely after these new weights are tuned).

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
  path-cost layer. Boldness reads at L2 (Patrol/Hunt/Fight `boldness`
  axis at `src/ai/scoring.rs:649`) and at the path layer
  (`crate::ai::pathfinding::cat_path_weight_from_boldness`). This
  ticket adds a *third* boldness-adjacent read site only if the
  follow-on conditioning is desired (out of scope here).
- After this ticket lands and weights are tuned, the cluster's
  intent is fully expressed: cats avoid choosing high-risk
  destinations (substrate L2 gate, this ticket) AND route around
  fox scent en route to non-fox destinations (path-cost overlay,
  223+224).

## Approach

1. For each of Hunt, Forage, Wander, Explore (one DSE at a time so
   each lands as a clean diff):
   1. Locate the DSE's `*Dse::new` constructor.
   2. Mirror `patrol.rs:98–123`:
      ```rust
      let fox_scent_weight = scoring.<dse>_fox_scent_weight.clamp(0.0, 1.0);
      if fox_scent_weight > 0.0 {
          considerations.push(Consideration::Scalar(ScalarConsideration::new(
              "fox_scent_level",
              Curve::Composite {
                  inner: Box::new(Curve::Logistic {
                      steepness: 6.0,
                      midpoint: 0.4,
                  }),
                  post: PostOp::Invert,
              },
          )));
          weights.push(fox_scent_weight);
      }
      ```
      Use the *exact* same curve constants as Patrol — composing
      well across the five DSEs requires shape parity. Per-DSE
      *weight* differs; per-DSE *curve* does not.
   3. Add `<dse>_fox_scent_weight` field to `ScoringConstants` with a
      `default_<dse>_fox_scent_weight() -> f32 { 0.0 }` function and
      `#[serde(default = "default_<dse>_fox_scent_weight")]` attribute,
      matching `patrol_fox_scent_weight`'s shape.
   4. Add the field to `ScoringConstants::default()` impl using the
      default function.
2. Tests per DSE (mirror `patrol.rs::tests`'s
   `axis_dormant_at_default_weight` / `axis_active_at_nonzero_weight`):
   - Dormant at default 0.0: CP composition contains exactly the
     pre-axis considerations.
   - Active at e.g. 0.3: CP composition contains the new
     `fox_scent_level` ScalarConsideration with the correct
     `Composite{Logistic{6.0, 0.4}, Invert}` curve and the assigned
     weight.
3. Document the decision-time vs route-time framing in each DSE's
   doc comment near the new axis. The L2 axis decides *whether* to
   pick that disposition in fox territory; the path-cost overlay
   (223+224) decides *where* the route runs. Future readers must
   not collapse them per the §4.7 boundary.

## Verification

- `just check && just test` — substrate ships dormant; existing
  tests pass; new per-DSE dormant/active tests pass.
- No soak required for the dormant ship — the L2 score paths are
  unchanged at `weight = 0.0`. Tuning ticket follow-on does the
  soak when it lifts the weights.
- After tuning lifts at least one weight non-zero, soak predictions
  should match the 223 regression analysis:
  - Adult Starvation deaths drop to 0 (cats avoid hunting in fox
    territory at decision time, so they don't get adrenaline-flee
    locked).
  - Courtship events return non-zero; `MatingOccurred` fires.
  - ShadowFoxAmbush deaths trend at-or-below post-223 levels (cats
    avoid fox territory at decision time).
  - `just frame-diff` between post-223 and post-tuning focal traces:
    Hunt / Forage mean scores DROP in fox-scent buckets ≥ 0.4; Eat
    / StockpileForage / Move scores RISE in the same buckets.
- Drift > ±10% on a characteristic metric requires a four-artifact
  hypothesis at `docs/balance/<N>-fox-scent-decision-axes.md` per
  CLAUDE.md balance discipline (in the tuning follow-on, not here).

## Log

- 2026-05-07: opened from 223's verification regression — soak showed
  3 adult Starvation deaths clustered in the [33,21]–[31,22] fox
  corridor + 0 courtship + never-fired Mating canaries. Theory of
  the case: 223's path-cost overlay is route-time substrate; the
  retired damp branch was decision-time substrate. They compose
  orthogonally; 223 alone leaves a gap at the decision layer.
- 2026-05-07: reframed via substrate-over-modifier discussion. Original
  draft proposed re-adding `FoxTerritoryHuntSuppression` as a
  `ScoreModifier`; that's exactly the override pattern the substrate
  refactor is retiring. Reframed scope to extend 209's L2 cost-axis
  pattern from Patrol to its four siblings (Hunt/Forage/Wander/Explore),
  shipping dormant at 0.0 with tuning as a separate follow-on.
