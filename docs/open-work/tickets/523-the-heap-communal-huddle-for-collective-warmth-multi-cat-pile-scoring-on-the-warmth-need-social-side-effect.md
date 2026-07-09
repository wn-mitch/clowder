---
id: 523
title: The Heap — communal huddle for collective warmth (multi-cat pile scoring on the warmth need, social side-effect)
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [welfare-fidelity, world-richness]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: [warmth-split]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The §5 sideways-broadening chart is overwhelmingly self- and dyadic behaviors;
the *collective* (colony-scale) region is nearly empty. The Heap fills the
Collective×Living cell: on a cold night cats pile together for shared warmth — a
communal huddle that is thermoregulation first and a social bonding side-effect
second. It is the honest-ecology thing cats actually do, it gives the warmth need
a collective satisfier distinct from seeking a fire or a nest, and it adds
colony-scale leisure texture to a world whose social behaviors are otherwise all
one- or two-cat. It does not light a currently-zero canary — grooming and play
already carry social variety — but it broadens §5 into the under-served
colony-scale region.

## Scope

- A `Huddle` affordance: a cat with unmet warmth (and low higher-tier need) can
  join or seed a huddle cluster at a location; co-present huddlers each receive a
  warmth benefit scaled by cluster size (diminishing).
- Scoring on the **existing** warmth need — no new need. Joining raises when
  warmth is low and a huddle (or a huddle-capable neighbor) is nearby.
- A social side-effect: time spent huddling contributes to the existing bond /
  affiliation substrate at a small rate — huddling reads as togetherness, not
  just heat. No new social axis.
- Join / leave / dissolve logic so a huddle forms as cold sets in and breaks up
  when warmth is met or a higher need preempts.
- Tunables (warmth-per-huddler curve, cluster cap, join threshold, bond rate) in
  `src/resources/sim_constants.rs`.

## Out of scope

- Redefining the warmth need itself — this is a *behavior* layer atop the warmth
  need, gated behind the `warmth-split.md` need refactor (see Dependencies).
- A new social/bond axis — couple to the existing one.
- Spatial nest/den construction — huddling happens at an existing location; it
  does not build anything (that is buildings-zones work).

## Current state

New. Not started. Provenance: surfaced by the `ideonomy` skill (2026-07-09 tuple
`dimension-identification + cross-domain-reinstantiation / chart /
animacy·autonomy·age`) as the empty Collective×Living cell — the
THERMO-BIOLOGY re-instantiation of "a pile is a shared heat sink" — the
colony-scale region the §5 chart flagged as under-served. Priced with
`rank-sim-idea`.

Rank-sim-idea triage (2026-07-09), anchored to shadowfox = 150:

| V | F | R | C | H | Score | Bucket     |
|---|---|---|---|---|-------|------------|
| 3 | 4 | 3 | 3 | 3 | 324   | worthwhile |

- V=3: fills the under-served collective-leisure region; social + warmth, but
  does not light a currently-zero canary.
- F=4: cats genuinely pile for heat; honest and emergent, no director.
- R=3: rides the warmth need and needs multi-cat spatial coordination — the
  coordination is the risk; A/B verification required.
- C=3: spatial clustering + warmth coupling + join/leave logic; ~700–1.2k LOC,
  new coordination but below GOAP-rework.
- H=3: couples warmth + social + coordination; warmth is already a tuned axis.
  Moderate measured-metric tax, no hard gate. (H-source: balance-grep — warmth
  ≈25 iterations, lower-tuned than food/grooming, so the coupling is tolerable.)

## Dependencies

Gate AFTER `warmth-split.md` lands so the huddle is scored against a stable
warmth-need definition rather than a moving target. Not a hard `blocked-by`
(no ticket id yet for the warmth-split refactor) — sequencing only.

## Approach

Reuse the existing JointIntention / co-present coordination substrate (tickets
127 / 277) for the join/leave/dissolve — a huddle is an N>2 co-located practice,
not a new coordination mechanism. Warmth benefit is a per-tick modifier while
co-present; the social side-effect is a small bond increment through the existing
affiliation substrate. Keep the warmth-benefit curve conservative on first ship
so a huddle can't trivialize the warmth need (that would destabilize the warmth
balance axis). All tunables in `src/resources/sim_constants.rs`.

## Verification

- Deterministic scenario: drop ambient temperature on a fixed seed with ≥3
  co-located cats; assert a huddle forms, each huddler's warmth recovers faster
  than a lone cat, and the huddle dissolves when warmth is met.
- `just verdict` on a cold-season soak: warmth-driven deaths unchanged or
  reduced; grooming/play canaries unaffected; no new balance canary required.
- Focal-cat trace: `Huddle` eligible and winning when warmth is low and
  neighbors are present; the warmth modifier and bond increment surface in the
  trace.

## Log
- 2026-07-09: opened from an `ideonomy` → `rank-sim-idea` generate-then-price
  pass over the §5 sideways-broadening axes. Scored 324 (worthwhile). Gate behind
  `warmth-split.md` so it isn't tuned against a moving need definition. Sibling
  tickets 521 / 522 / 524 from the same pass.
