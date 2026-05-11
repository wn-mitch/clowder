---
id: 219
title: shared recent-ambush event marker
status: done
cluster: ai-substrate
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-11
---

## Why
210's mechanism investigation showed ambushes cluster spatially:
60–70% of ShadowFox attacks land in 2–3 tile zones near the colony
center ([20–29, 20–29] hot zone takes 7–8 ambushes per 15-min soak),
and temporally (Calcifer's 5 ambushes in 941 ticks killed him in
baseline; Thistlekit's 3 ambushes in 2489 ticks preceded his
starvation in post-210). The colony's perception substrate already
has `FoxScentMap::base_sample` and `colony_tension_recent` (added by
209), but neither anchors on **where ambushes have actually
happened recently**. Cats currently treat every tile as
predator-equivalent until they get jumped.

The colony has shared-knowledge infrastructure (`Coordinator`
DirectivesIssued = 32k+ events per soak, `WardPlaced` events). What
it lacks is a **per-tile temporal-decay scalar of recent ambush
events** — so when Wren gets ambushed at [29,23], every other cat's
perception of that tile gets elevated cost for the next ~5k ticks.

## Scope
- New scalar in `src/systems/interoception.rs` (or a new
  influence-map resource if grid-shaped is more natural):
  `recent_ambush_at_position` — exponential temporal decay (~5k-tick
  half-life) over `Ambush` events, sampled at cat position.
- Ships dormant — read-sites land in same commit but with
  weight=0.0 placeholders, per substrate-stub forbiddance rule.
- Pre-existing `Ambush` event (already emitted at
  `src/systems/predators.rs` or wherever ShadowFox combat resolves)
  is the input — no new event types needed.

## Out of scope
- Per-cat tile memory (different from colony-shared) — separate
  follow-on if useful.
- Wiring to specific DSEs (Patrol, Forage, Caretake) — that's
  ticket 220 (ward-placement) and ticket 221 (caretake-relocate)
  consuming this scalar; this ticket just builds the substrate.
- Ambush by other predators (Hawks, Snakes) — start with ShadowFox,
  generalize if soak shows other predators clustering similarly.

## Current state
210 closeout (sha `bdfec651cd28`) documented the empirical clustering.
214 (`patrol_fox_scent_weight`) is the closest active sibling — it
turns on `FoxScentMap` reading on Patrol; this ticket adds a
*different* perception (event-anchored, not scent-anchored), and
they compose.

## Substrate posture (alignment with 249)

`RecentAmbushMap` joins `RecentDispositionFailures`,
`RecentTargetFailures`, and `HuntingPriors::record_failed_search`
as a **typed-failure / typed-event memory substrate** — a per-flavor
data structure with tick-decay, consumed by a specialized reader
function, fed into `ScoringContext`. Per §12.1 of
`docs/systems/ai-substrate-refactor.md`, the substrate has no
general memory→scoring coupling today; each typed-failure
component is a temporary proxy until Talk-of-the-Town's unified
`Memory` consumer lands (cluster C3, ticket 007).

This ticket ships `RecentAmbushMap` as designed — the colony-shared
spatial event memory is genuinely orthogonal to the per-cat
per-disposition cooldowns and serves a load-bearing perception
need 210 documented (60–70% of ambushes cluster spatially). But
authoring it is **adding a 4th typed-failure substrate**, and the
closeout `## Log` line on land MUST cite the C3 consolidation path
explicitly so future readers see the substrate as a temporary
proxy, not a permanent fixture.

When C3 lands, `RecentAmbushMap` should fold into the unified
mental-model substrate as a `LocationModel.last_threat` facet
(per 007's "Mental model facets" — *"For location mental models:
**last_threat** (fox at this tile three days ago)"*) with
proper evidence typology. Until then: the marker stands.

249's reframe applies the same "don't grow new typed-failure surface
area" doctrine to `DispositionFailureCooldown`'s match arms; the
analogous question for 219 ("is `RecentAmbushMap` the right shape?")
is answered "yes for now, with an explicit retirement path." See
`src/ai/modifier.rs::DispositionFailureCooldown` rustdoc and §3.5.5
for the boundary documentation.

## Approach
1. Add `RecentAmbushMap` resource (grid-shaped, decaying float per
   tile) registered with `populate_influence_map_registry` per
   ticket 207's enforcement.
2. System `update_recent_ambush_map` runs each tick: decays values
   by `exp(-Δt / half_life)`; on `Ambush` event read, set the tile
   value to 1.0 (or accumulate, capped at 1.0).
3. Sampler in `interoception.rs`:
   `recent_ambush_at_position = RecentAmbushMap::base_sample(cat_pos)`.
4. Add to `ScoringContext` for L2 evaluation.
5. Constant `recent_ambush_half_life_ticks` (default ~5000) in
   `sim_constants.rs`.
6. Ship dormant — no DSE reads it yet. Same-commit reader: a unit
   test that verifies the sampler returns non-zero when an ambush
   was logged near the position.

## Verification
- `just check` (substrate-stub + influence-map registry lints pass)
- `just test` (new unit test for the scalar's decay shape)
- Trace inspection: `just soak-trace 42 Wren` and confirm
  `recent_ambush_at_position` appears in trace records with
  spatial+temporal variation (zero far from ambush sites; non-zero
  within 5k ticks of an ambush event near Wren's position).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed ** 14** (done, —, score 0.90 (cross-cluster)) — "§4.2 State marker trio — `InCombat` / `OnCorruptedTile` / `OnSpecialTerrain` a…
- · **173** (parked, ai-substrate, score 0.88) — IsHerbalist / IsSpiritualist / HasCorruptionNearby capability markers (155 foll…
- ✓ landed ** 49** (done, —, score 0.88 (cross-cluster)) — §9.2 faction overlay markers

<!-- linkages:end -->
## Log
- 2026-05-07: opened from 210 closeout.
- 2026-05-11: 2026-05-11: landed. RecentAmbushMap is a typed-failure proxy joining RecentDispositionFailures / RecentTargetFailures / HuntingPriors::record_failed_search; per §12.1 of ai-substrate-refactor.md, folds into the unified Memory.LocationModel.last_threat facet when ToT cluster C3 (ticket 007) lands. Ships dormant — registered in InfluenceMapRegistry, deposited inline in predator_stalk_cats, decayed exponentially via update_recent_ambush_map (half_life=5000 ticks). No DSE consumes the scalar yet; tickets 220 (ward-placement) and 221 (caretake-relocate) will. Soak-trace 42 Wren verified: 109k non-zero L1 samples across 122k ticks of activity, max 1.0, decay shape correct. Zero behavioral drift vs baseline (every footer field delta_pct=0.0).
