---
id: 262
title: Audible-cue range falloff modeling — distance attenuation for alarm calls and distress cries
status: blocked
cluster: belief-perception
orchestration: substrate-sensitive
initiative: [full-sensory-perception]
added: 2026-05-10
parked: null
blocked-by: [244]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 244 lands the audible-cue substrate (alarm calls, distress cries, hissing) as L1 markers other cats can read. At v1 those cues are flat-range — within audible distance, fully present; outside, absent. Real audio attenuates with distance: a cry next door is a 1.0 signal; the same cry across the colony is barely above noise floor. Without falloff modeling, distant cats over-weight far-off cries (false alarms) or under-weight near-by cries (no urgency gradient), which corrupts downstream consumers (the C3 belief integrator at ticket 258 reads audible cues as evidence of `recency_of_threat_cue` and other facets — flat-range presence makes for noisy belief updates).

Adjacent to the C3 spinout cluster (256→258) but independent: audible cues exist before C3, and C3 will consume them whatever range model they have. Adding falloff here is a perception-honesty improvement that benefits both pre-C3 and post-C3 consumers.

## Scope

- **Distance-attenuation curve per cue type**: hissing, alarm call, distress cry each get a falloff curve. Inverse-square is the obvious starting point; physical realism isn't the goal — perception honesty is.
- **Per-cue intensity at source**: alarm calls (loud, broadcast) > distress cries (medium) > hissing (quiet, intimate). Source intensity multiplies falloff.
- **Reader-side `audible_strength: f32` per cue**: instead of "in range / out of range" binary, readers get a `[0, 1]` strength scaling with `intensity_at_source × falloff(distance)`. Threshold below which strength = 0 (so far-away cries don't add per-tick perception cost).
- **Obstacle attenuation (v2 / out of scope)**: walls, water, foliage. v1 is line-of-sight free-air falloff only. Obstacles deferred.

## Out of scope

- The audible-cue substrate itself (ticket 244 owns).
- Visual-cue range falloff (ticket 243 owns body-cue reads; visual range model is its concern).
- Sound occlusion / reverberation / directional hearing.
- Multi-cue summation rules (one cat hissing + one cat alarm-calling — does the listener perceive these additively, max-of, or some other composition? Probably worth its own ticket once two consumers exist that disagree).

## Current state

- Blocked-by 244 (audible-cue substrate). 244 currently lists hissing/alarm/distress-cry as v1 cues; until 244 lands, this ticket has no substrate to attenuate.
- Falloff curves can be defined at design time even before 244 ships; this ticket can co-design with 244 to ensure the v1 reader API can carry an `audible_strength` field (rather than a boolean) from the start, avoiding a v2 migration.

## Approach

1. Coordinate with 244 to ensure the cue-emission API carries source intensity from the outset.
2. New `audible_falloff` helper in `src/systems/sensing.rs` (or wherever 244 puts the audible-cue read) that maps `(intensity_at_source, distance, cue_type)` → `audible_strength`.
3. Per-cue tunables in `SimConstants`: `intensity_at_source`, `falloff_exponent`, `audible_strength_floor`.
4. Update the cue read site to populate `audible_strength` instead of (or alongside) the binary "in range" flag.

## Verification

- Scenario: emit alarm call at position A; verify reader at distance d=1 perceives strength ~1.0, at d=10 perceives strength ~0.1, at d=20 perceives strength below floor (treated as zero).
- Scenario: emit distress cry vs alarm call at the same position; verify alarm call carries further (higher source intensity).
- Soak: no behavioral regression on existing cue consumers (244's verification scenarios should pass identically post-falloff for nearby cues; far-away false-positives should disappear).

## Log

- 2026-05-10: opened as parking-lot perception-improvement adjacent to the C3 spinout cluster (ticket 258). Independent; blocked-on 244.
