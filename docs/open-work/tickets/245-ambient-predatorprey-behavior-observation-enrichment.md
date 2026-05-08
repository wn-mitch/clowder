---
id: 245
title: Ambient predator/prey behavior-observation enrichment
status: blocked
cluster: null
added: 2026-05-08
parked: null
blocked-by: [243]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
126 lands the actor-private `HeldIntention` substrate; the same
body-cue + behavior-observation channel that powers care delegation
also enriches predator/prey interactions, but the consumer DSEs
(ShadowFox `Hunt`, cat `Flee`) are out of 126's scope. A ShadowFox
displaying `LimpingGait` is approachable; one with `StalkingPosture`
is fled; a kitten with `HeadDownCurled` is easier to ambush than an
alert sentinel cat. These reads make predator/prey strategy emergent
from observable state rather than hard-coded.

## Scope
- Predator DSEs read target body-cue + physical markers via 243's
  channel: cat `StalkingPosture` near a ShadowFox alters approach
  axis; cat `HeadDownCurled` raises ambush viability.
- Prey DSEs (cat `Flee`) read predator body-cues: a `LimpingGait`
  ShadowFox produces lower threat-derivative than a healthy one.
- Wildlife-side body cues authored alongside the read sites —
  substrate-over-override at the L1 surface.

## Out of scope
- Cross-species communication / mixed-faction coordination.
- Predator personality variation — separate balance iteration.

## Current state
Blocked-by 243 (behavior-observation channel) and transitively 242.

## Approach
Add new body-cue ZST markers for wildlife species; wire predator/prey
DSE axes to read them via 243's channel.

## Verification
- ShadowFox approach axis differentiates between alert and
  oblivious cats in focal traces.
- Cat Flee axis differentiates between healthy and limping
  ShadowFoxes.

## Log
- 2026-05-08: opened on 126's C4 landing commit.
