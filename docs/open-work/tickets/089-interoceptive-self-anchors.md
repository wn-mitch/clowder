---
id: 089
title: Interoceptive self-anchors — spatial self-perception (OwnInjurySite, OwnSafeRestSpot)
status: ready
cluster: ai-substrate
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

087 deliberately publishes scalar interoception only (urgencies + a composite distress scalar). The architectural symmetry isn't complete until interoceptive perception also publishes *spatial* anchors the way external perception publishes `LandmarkAnchor::NearestThreat`, `LandmarkAnchor::OwnSleepingSpot`, etc. Cats need to be able to navigate to a body-state-appropriate location — a safe rest spot tuned to *this* cat's preferences, an injury locus they're trying to keep weight off, etc.

DSE-level use cases this unblocks:
- `Rest` DSE gains a `SpatialConsideration` over `LandmarkAnchor::OwnSafeRestSpot` (analog of `Sleep`'s `OwnSleepingSpot`) so resting near a kitchen / hearth scores higher than resting next to a den entrance, all else equal.
- A future `TendInjury` DSE (separate ticket) needs `OwnInjurySite` to know where on its own body the injury locus is, so navigation to a healer / herb cache makes sense.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)) — extends 087's substrate to spatial self-perception, on-thread but not directly retiring an override.

**Hack shape**: not a hack-retirement directly — this ticket is *substrate expansion*. It completes the architectural symmetry 087 left incomplete (scalar interoception only). Without spatial self-anchors, future TendInjury / Rest-near-safety DSEs would have to use override-shaped target-resolvers to reach body-state-appropriate locations.

**IAUS lever**: `LandmarkAnchor::OwnInjurySite`, `LandmarkAnchor::OwnSafeRestSpot`, possibly `OwnTerritoryCenter` — first-class spatial-self-perception axes consumed via `SpatialConsideration` in self-care DSEs. Mirrors external perception's `LandmarkAnchor::NearestThreat` etc.

**Sequencing**: blocked-by 087 (landed). No prerequisite for 047 specifically, but lands on the same substrate-expansion thread.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

## Scope

- Extend `LandmarkAnchor` enum (`src/ai/considerations.rs`) with `OwnInjurySite`, `OwnSafeRestSpot`, possibly `OwnTerritoryCenter`.
- Author the anchors from interoceptive perception module (087) — read from `Health.injuries`, `Memory` of past safe rests, etc.
- Update `src/ai/dses/rest.rs` to add `OwnSafeRestSpot` SpatialConsideration (Power-Invert curve over distance, mirroring `OwnSleepingSpot` in Sleep).
- `OwnInjurySite` lands without a consumer in this ticket — its consumer is a future `TendInjury` DSE in the L2.10 catalog enumeration.

## Verification

- Unit test: anchor authoring produces consistent positions for cats with stable Health/Memory state across ticks.
- Focal-cat trace: a wounded cat near a safe-rest spot picks Rest with a higher score than the same cat far from a safe-rest spot.

## Out of scope

- The `TendInjury` DSE itself — separate ticket in L2.10 catalog enumeration.
- Memory-based safe-rest learning (cats remembering which spots they've slept safely at) — that's a separate persistent-component ticket; this ticket can stub `OwnSafeRestSpot` to "current home tile" until memory lands.

## Log

- 2026-04-30: Opened alongside 087. Blocked-by 087 (perception substrate) until that lands.
