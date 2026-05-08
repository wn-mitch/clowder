---
id: 243
title: Behavior-observation L1 channel (target-side body-cue + physical marker reads)
status: blocked
cluster: null
added: 2026-05-08
parked: null
blocked-by: [242]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
126's narrative-discipline framing requires sister DSEs to read body
cues on other entities; today behavior-observation queries are
ad-hoc per-DSE rather than a unified L1 surface. Without a shared
read pattern, every consumer reinvents the per-target marker
+ position lookup, which fragments invariants across DSEs.

## Scope
- A target-side query pattern formalising how an actor's L2 reads
  another entity's body-cue markers + physical markers + position.
- Extension of the `MarkerSnapshot` shim (or a sibling) so
  `EligibilityFilter::require(marker)` and DSE custom predicates
  can consult target-entity markers, not just self-markers.
- Reusable axis helpers for "candidate has cue X within radius R"
  so 245 (predator/prey) and 129 (Care DSEs) consume one surface.

## Out of scope
- Specific consumer DSEs — Care (129), predator/prey (245),
  joint-intention (127) are all separate tickets.
- Sensory attenuation / detection-range scaling — owned by §5.6.6.

## Current state
Blocked-by 242 (body-cue substrate). Sister DSEs can't read cues
that don't exist yet.

## Approach
Extend the pattern that `MarkerSnapshot` already establishes for
self-markers to target entities. Either reuse the snapshot with a
target-keyed surface or author a sibling `TargetMarkerSnapshot`.

## Verification
- At least one consumer DSE reads target body-cues through the new
  channel.
- L2 trace shows the target-cue input as a labelled axis.
- No per-DSE ad-hoc target marker queries remain.

## Log
- 2026-05-08: opened on 126's C4 landing commit. Blocks 245.
