---
id: 515
title: cat-vs-cat aggression substrate — disposition, plan template, resolver for intra-colony hostile escalation beyond the 267 Threaten/Posture/Hiss rungs (0.5.0 target; unblocks 304)
status: ready
cluster: combat-threat
orchestration: substrate-sensitive
initiative: []
added: 2026-07-07
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Cat-vs-cat aggression does not exist: `resolve_combat` is exclusively
cat-vs-wildlife, there is no hostile disposition/plan-template/
resolver targeting another cat, and ticket 304's
`WitnessableEvent::Attack` emit sleeps as a reader-without-writer
because of it. The 0.4.0 plan (step 16) descopes 304 behind this
prerequisite: designing WHEN cats attack each other (escalation past
267's Threaten / Posture / Hiss rungs, territory antagonism per 268,
banishment enforcement) is a design surface of its own, targeted
0.5.0. This ticket owns that substrate; once a real resolver exists,
304 wires the Attack emit on top in one commit.

## Scope
- Disposition + plan template + resolver for intra-colony hostile
  escalation (the rung ABOVE 267's conflict-low DSEs).
- Escalation gating: aggression must be reachable only through the
  267 rungs (posture → threaten → strike), never a first resort —
  compose personality (temper, boldness), relationship state, and
  the 258 belief substrate (perceived_violence_capability,
  perceived_hostility) at the eligibility/scoring layer.
- Injury pipeline reuse: strikes route through the existing body-zone
  / injury substrate (095), not a parallel damage path.
- Ethological grounding: displacement, resource-guarding, and
  banishment enforcement are the naturalistic triggers — confirm
  interpretation with the user before implementation (per the
  ethological-examples feedback discipline).

## Out of scope
- 304's Attack emit itself (one-commit follow-on once this lands).
- 267 (Threaten/Posture/Hiss) and 268 (territory antagonism) — this
  ticket sits above them and consumes their signals.
- Any 0.4.0 landing — targeted 0.5.0 per the release plan's step-16
  descope ceremony.

## Current state
Opened from the 0.4.0 plan's Phase III step 16 (descope ceremony).
304 re-blocked on this ticket. Adjacent design material: 267
escalation rungs, 268 perimeter antagonism, 258 belief facets
(perceived_hostility / perceived_violence_capability already carry
the perception side).

## Approach
Design-first: a short design stub under docs/systems/ before code
(escalation ladder, trigger taxonomy, injury-severity band vs
wildlife combat). Then the standard substrate order — disposition +
eligibility (dormant), resolver, activation via four-artifact soak.

## Verification
- Scenario: resource-guarding escalation reaches a strike only after
  the 267 rungs fire in order; a low-temper cat never escalates.
- Soak: intra-colony injuries appear at a rare, tunable rate;
  survival gates hold; 304's emit (follow-on) feeds the belief
  substrate without new stub warnings.

## Log
- 2026-07-08: opened from the 0.4.0 release plan step 16 (304
  descope ceremony). 0.5.0 target.
