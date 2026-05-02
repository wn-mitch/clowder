---
id: 124
title: `LandmarkAnchor::OwnTerritoryCenter` — third interoceptive self-anchor (territory bias)
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

089 (`landed-at: <089 sha>`) shipped two interoceptive self-anchors — `OwnSafeRestSpot` and `OwnInjurySite` — and explicitly held `OwnTerritoryCenter` out of scope under substrate-over-override discipline ("don't author unused substrate; pull it in when a real consumer arrives"). 089's §Out of scope flagged that follow-on tickets must open in the lands-day commit per the CLAUDE.md "Antipattern migration follow-ups are non-optional" convention. This is that ticket.

The third self-anchor — *where on the map is the cat's territorial center* — is the substrate that future Patrol-with-territory-bias / TerritoryDefense / Settle behaviors compose against. Without it, those DSEs will reach into per-cat queries directly inside the scoring path (override shape) instead of declaring `LandmarkSource::Anchor(LandmarkAnchor::OwnTerritoryCenter)` and letting the §L2.10 substrate resolve it (composable shape).

The blocker today: there is no consumer DSE in the catalog that needs a per-cat territorial center, and authoring substrate without a consumer was the same antipattern 089's stub was correcting for `OwnInjurySite` (which 089 then pulled in *with* an integration-test consumer to satisfy the discipline). For `OwnTerritoryCenter` we wait for the first real consumer.

## Substrate-over-override pattern

Substrate expansion on the substrate-over-override thread (093). The eventual hack-retirement is "Patrol DSE reaches into territory queries directly to bias toward home range" — a future override-shape this anchor pre-empts.

**IAUS lever**: `LandmarkAnchor::OwnTerritoryCenter` consumed via `SpatialConsideration` by Patrol / TerritoryDefense / Settle. Mirrors 089's `OwnSafeRestSpot` shape: per-cat dynamic anchor populated at `ScoringContext` construction.

**Canonical exemplar**: 089 (interoceptive self-anchors). Same layered shape — variant in `LandmarkAnchor` enum + `CatAnchorPositions` field + resolver arm + authoring helper in `interoception.rs` + DSE wiring at consumption time.

## Scope

Open-ended until a consumer is identified. Two paths:

1. **Consumer-pull (preferred).** Some future ticket — Patrol-with-territory-bias, a TerritoryDefense DSE, a Settle / homecoming behavior — names `OwnTerritoryCenter` as the substrate it needs. That ticket pulls 124 in as a dependency or rolls the variant + authoring into its own scope (089's pattern). At that point 124 either lands as the substrate half of the larger ticket or closes as `superseded-by`.
2. **Substrate-first (only if a clear authoring rule emerges).** A new ZST marker or `Memory`-derived helper computes the territory center from observable state (e.g., centroid of recent firsthand `MemoryEntry::location`s tagged with non-Sleep, non-Threat events; or per-cat `TerritoryClaim` component if the colony introduces explicit territories). The variant + resolver land first, with an integration test exercising the resolver, and a follow-on ticket adds the consumer DSE.

## Reproduction / verification

`just check` (no code change while parked).

When unparked: same layered verification pattern as 089 — pure-fn unit tests for the authoring helper, struct-field round-trip integration test, focal-cat trace showing the DSE that consumes it scores higher when the cat is near its territorial center.

## Out of scope

- Choosing the authoring rule. That's part of the substantive design once a consumer emerges.
- The Patrol DSE territory-bias behavior itself. Behavior is a separate ticket; this one is the spatial encapsulation.

## Log

- 2026-05-01: Opened by 089's land commit, per the antipattern-migration follow-up convention codified in `CLAUDE.md` §Long-horizon coordination. Status `ready` rather than `blocked` because the substrate-first path (option 2 above) is unblocked at any time; status will flip to `blocked-by: [<consumer-ticket-id>]` if a downstream consumer ticket lands first and pulls this one in as a dependency.
