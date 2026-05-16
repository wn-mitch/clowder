---
id: 368
title: Phase 2 behavioral tools — Grooming Brush, Play Bundle, Courtship Gift (016 Phase 2)
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: [365]
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Land three Phase 2 behavioral-tool recipes — Grooming Brush, Play Bundle, Courtship Gift — that target the §5 continuity canaries (grooming, play, courtship). All three craft at the existing Workshop. Effects live on the action resolver keyed to item identity, per `docs/systems/crafting.md` §Design constraints. Parent epic: [016](016-crafting-items-recipes-stations.md).

## Scope
- Three new `Recipe` entries on the existing Workshop station (no new structures).
- Resolver updates: grooming action reads brush presence and raises grooming output; play action reads play-bundle as a target object with higher kitten play-need satisfaction; mating chain reads gift presence on the courting cat and the fondness resolver factors gift type/quality.
- Inputs per design doc: Twig + prey-shedding Bristle (Grooming Brush); Fiber + Feather (Play Bundle); Polished Stone / Feather / Flower (Courtship Gift).

## Out of scope
- Warrior's kit (→ 369).
- Wearables on slot-inventory (→ 370 + ticket 017).
- Decorations or Phase 5 (→ 371 / 372).

## Approach
See `docs/systems/crafting.md` Phase 2. Hypothesis: post-368, grooming / play / courtship action counts each rise ≥1× per soak on seed 42 (from currently-zero or near-zero baseline).

## Verification
- `just hypothesize <spec.yaml>` runs the four-artifact treatment-vs-control on the three §5 continuity canaries.
- `just verdict <run-dir>` shows grooming / play / courtship each fire ≥1×.

## Log
- 2026-05-16: opened as 016 epic decomposition (Phase 2; parent 016, blocked-by 365).
