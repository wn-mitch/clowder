---
id: 508
title: Cat routing is threat-belief-blind — LocationBeliefs threat cues never feed path overlays; six serial deaths at the shadowfox haunting ground under A-star-first stepping (493)
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
The 493 verification soak (`logs/tuned-42-07acc090`) lost 6 of 8 adults
to ShadowFoxAmbush at ONE location box ([24-32, 58-64] — the
shadowfox's own haunting ground: 2 spawns, 236 HauntingEntered, 213
SeedingEntered inside it), including the pregnant queen Mocha at
gestation tick 6381/20000 → kittens_born 0 for the run. Serial deaths
at a fixed point over 70k ticks with zero route adaptation: the colony
witnessed five deaths there and kept walking in. Position scan: cat
snapshot-presence in the box doubled under 493 (0.8% → 1.8%) — A*-first
stepping concentrates traffic onto optimal corridors, and this corridor
crosses a lair.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Threat memory | 258 belief substrate: `LocationBeliefs.recency_of_threat_cue` per bucket (the retired `RecentAmbushMap`'s designed successor per belief_aggregation.rs) | per-cat, witnessed-event-fed; READ by scoring scalars (`recent_ambush_at_position`, `patrol_threat_recency`) | `[verified-correct]` |
| Routing overlays | `pathfinding.rs`: FoxScentOverlay, CorruptionOverlay, CatPatrolDeterrentOverlay(fox-side) | **no overlay reads threat beliefs** — cat A* is threat-blind; scoring knows, feet do not | `[verified-hostile]` (the gap) |
| Route concentration | 493 A*-first `step_toward` + (upcoming) smoothed corridors | all movers take the same optimal corridor; presence-in-box x2.3 | `[verified-*]` via position scan above |
| Fox side | shadowfox haunts a fixed corruption ground; wards placed there (4) get despawned; `ward_count_final = 0` | the colony fights the lair with wards but bleeds bodies on approach | `[verified-correct]` |
| Fertility coupling | agent sweep: courtship→mating chain INTACT (1 mating fired; pregnancy cut by the hotspot death; breeder pool then zeroed) | fertility zero is downstream of this ticket, not a mating bug. Chronic Mating commitment-thrash (bonded pairs replan Mate away tick-for-tick) noted separately below | `[verified-correct]` |

## Fix candidates
- R1 (parameter) — raise ward pressure / fox repel constants. Treats
  the symptom; cats still cannot learn a place is lethal.
- R2 (**structural — belief-fed routing overlay**) — new
  `ThreatBeliefOverlay` (`TileCostOverlay`) reading the ROUTING cat's
  own `LocationBeliefs.recency_of_threat_cue` (and/or `threat`-family
  facet) at each tile's bucket, scaled by a new
  `threat_belief_path_cost_max` constant (FoxScentOverlay precedent:
  clamp × max_cost). Subjective by construction — only cats that
  witnessed/heard of deaths detour (Bayes-flavored beliefs per the
  perception-as-beliefs doctrine; NOT a resurrection of the retired
  colony-shared RecentAmbushMap). Wire into the cat overlay slices in
  goap.rs/disposition.rs travel + chase paths, weight conditioned by
  boldness like the existing `cat_path_weight_from_boldness`.
- R3 (extend) — also price threat beliefs into ward-placement TARGET
  choice (cats placed 4 wards inside the lair box). Defer unless R2's
  soak still shows ward-mission deaths dominating.

## Recommended direction
R2. Same shape as the existing overlay family, uses the substrate the
258 refactor built for exactly this, and directly counters the route-
concentration property every remaining Phase II landing amplifies.
Lands BEFORE plan step 6 (the integrator consumes the same overlay
slice via smoothed corridors) — pillar 2 ordering.

## Out of scope
- Mating commitment-thrash for bonded pairs (Mate plans displaced
  tick-for-tick by Cook/Explore before TravelTo closes; chronic,
  pre-dates 493, pillar-4 family). Open/fold into a commitment-layer
  ticket if the post-508 soak still shows zero conversions from
  BONDED pairs.
- Shadowfox-side satiation/den/memory — 310 S1-S5 (Phase V).

## Verification
Four-artifact soak vs the post-506 baseline: predictions —
ShadowFoxAmbush deaths < 6 (ideally 0-2), presence-in-box rate back
under ~1%, kittens_born >= 1, canaries green, throughput within
+/-15% of the 493 run. Frame-diff: Patrol/Flee shape stable (the
overlay is routing-layer, not scoring-layer).

## Log
- 2026-07-05: opened from the 493 landing gates (/diagnose-collapse
  three-agent sweep + position scans; evidence tables above).
