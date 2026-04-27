---
id: 050
title: §4 marker predicate refinements — species-attenuated threat, ward-near-fox truth, event-driven cubs/den
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, sensory.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The §4 marker catalog large-fill (ticket 014, landed 2026-04-27)
authored 19 §4.3 markers as bit-for-bit mirrors of pre-existing
inline computations. Three of those mirrors were intentionally
simplified or stubbed during the marker port to keep the cutover
behavior-neutral; this ticket promotes them to their truthful
predicates so any balance impact is isolated to a single commit.

## Scope

Three predicate refinements, all small:

### 1. `HasThreatNearby` — species-attenuated detection range

Today's mirror at `sensing::update_target_existence_markers` uses a
flat `wildlife_threat_range` Manhattan-distance scan
(`disposition.rs:643–648` legacy). The §4.2 spec calls for
species-attenuated detection — different cat sensory profiles for
fox / hawk / snake / shadowfox. Promote the predicate to use
`observer_sees_at` with the `WILDLIFE` signature and species-aware
ranges from `SensoryConstants`.

### 2. `WardNearbyFox` — truthful predicate

Today's author at `fox_spatial::update_ward_detection_markers` returns
`false` unconditionally to mirror the pre-existing
`FoxScoringContext.ward_nearby = false` stub. Promote to a truthful
"any ward within fox detection radius" check. No DSE consumes
`WardNearbyFox` today (the cutover is deferred — see ticket 051), so
this refinement is staging for future fox-flee-from-wards behavior.

### 3. `HasCubs` / `HasDen` — event-driven authoring

Today's per-tick scans at `fox_spatial::update_cub_marker` and
`update_den_marker` are simple but iterate every fox every tick.
The marker rustdocs nominate event-driven authoring:
- `HasCubs` ← `CubsBorn` insert; on-cub-despawn cleanup.
- `HasDen` ← `DenClaimed` insert; `DenLost` remove.

The `CubsBorn` / `DenClaimed` / `DenLost` events don't exist yet.
Define them, emit at the right sites in `fox_lifecycle_tick` /
fox-action resolvers, and migrate the authors to event-driven.

## Out of scope

- Fox DSE eligibility migration (covered by ticket 051).
- Cleanse / Harvest / Commune spatial-target routing — that's a
  §6.3 follow-on, not a marker-predicate refinement.

## Approach

Each refinement is a separate commit. Per CLAUDE.md balance
methodology, drift > 10% on a characteristic metric requires a
hypothesis. Soak verdict expected:
- Threat-nearby refinement: cats may detect threats slightly later
  for non-fox wildlife (depending on profile tuning). Hypothesis:
  marginal shift in `ShadowFoxAvoidedWard` / Flee firing.
- WardNearbyFox: zero soak delta (no consumer reads it).
- Cubs/Den event-driven: zero soak delta (predicate semantics
  unchanged; only authoring cadence changes).

## Verification

- Lib tests: ~5 tests per refinement.
- `just check` green per commit.
- Soak verdict + per-metric fingerprint diff against
  `logs/baseline-2026-04-25/` baseline.

## Log
- 2026-04-27: opened from ticket 014 closeout (§4 marker catalog
  large-fill). Three predicate-refinement follow-ons surfaced
  during the marker port: species-attenuated threat detection,
  truthful ward-near-fox predicate, event-driven cub/den authoring.
