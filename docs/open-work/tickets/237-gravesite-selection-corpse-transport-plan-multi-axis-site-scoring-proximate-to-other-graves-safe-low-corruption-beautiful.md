---
id: 237
title: Gravesite selection — corpse transport plan + multi-axis site scoring (proximate to other graves, safe, low-corruption, beautiful)
status: ready
cluster: buildings-zones
initiative: [mythic-texture]
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

035 ships burial-at-corpse — the corpse is despawned and a `Grave` is
spawned at the same tile. The user's vision wants cats to **choose a
gravesite**: cluster graves into emergent cemeteries (proximate to
other graves), away from danger (low ShadowFox influence), out of
corrupted tiles (low corruption), and on beautiful tiles (terrain +
flora richness).

The "proximate to other graves" axis is structurally important: it
gives cemeteries an emergent shape. Kittens-rest-at-grave (239)
works better when graves are clustered.

## Scope

- New `gravesite_picker` target-taking DSE — scores candidate sites
  with the four axes above.
- Bury plan template extends from `[Bury]` (035) to `[GatherCorpse,
  TransportToSite, BuryAtSite]`. The cat carries the corpse from its
  original position to the picked gravesite.
- New planner zone `PlannerZone::Gravesite` (or extend `CorpseTarget`
  to disambiguate "the corpse" from "where to put it").

## Out of scope

- Body preparation tiers (236).
- Ceremony richness (238).
- Kitten-rest-at-grave chain (239) — depends on this ticket's grave
  clustering but is its own scope.

## Approach

Score axes mirror the ward-placement DSE shape (anti-clustering for
wards, pro-clustering for graves). The "beautiful" axis can read
from `Terrain` (Grass + Flowers > BareDirt) and existing flora-density
maps.

## Verification

- Scenario: `gravesite_clustering` — three deaths in succession
  produce three graves clustered within ~5 tiles of each other (vs.
  scattered across the map under naive nearest-tile picking).

## Log

- 2026-05-08: opened as 035 follow-on.
