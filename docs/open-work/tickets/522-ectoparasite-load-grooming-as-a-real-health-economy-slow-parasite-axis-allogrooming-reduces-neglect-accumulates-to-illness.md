---
id: 522
title: Ectoparasite load — grooming as a real health economy (slow parasite axis allogrooming reduces, neglect accumulates to illness)
status: ready
cluster: social-coordination
initiative: [welfare-fidelity, world-richness]
orchestration: substrate-sensitive
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

§5 names axis #1 as "grooming as a social **and health** economy" — but today
only the social half exists. Grooming (self- and allo-) scores as a social /
mood behavior with no material health stake, so the "health economy" half of the
stated axis is unbuilt. Ectoparasite load supplies it: a slow-accumulating
per-cat parasite axis that self-grooming trims and allogrooming trims *for a
bondmate*, while a neglected cat's load climbs until it becomes an illness
pressure. This completes a §5 axis the vision doc explicitly calls for, turns
allogrooming into honest mutual care with a real payoff, and gives the grooming
canary a reason to fire that isn't purely cosmetic — all as an ecological
pressure with no director thumb.

## Scope

- A slow per-cat `ParasiteLoad` axis (interoception-style scalar) that rises on a
  slow decay clock and is reduced by grooming actions — self-grooming trims own
  load, allogrooming trims the *target's* load.
- A grooming read-site: the existing grooming DSE gains a consideration that
  raises grooming drive as own (or a bondmate's) load climbs, so cats groom the
  neglected — care routed by need.
- An illness coupling at the high end: sustained high load feeds an existing
  health/illness pressure (couple to it rather than adding a parallel death
  cause).
- Tunables (accrual rate, per-groom trim, bondmate-vs-self weighting, illness
  threshold) in `src/resources/sim_constants.rs`.

## Out of scope

- A full disease/contagion model (transmission between cats, epidemics) — load
  is per-cat and self-contained; contagion is a separate, larger ticket.
- A new death cause if a health/illness axis already exists — couple to it.
- Body-zone / anatomical injury (that is ticket-line body-zones.md, distinct).

## Current state

New. Not started. Provenance: surfaced by the `ideonomy` skill (2026-07-09 tuple
`dimension-identification + cross-domain-reinstantiation / chart /
animacy·autonomy·age`) as the IMMUNOLOGY re-instantiation of the Dyadic×Living
cell — "grooming is parasite control" — deepening an existing §5 axis rather
than filling an empty cell. Priced with `rank-sim-idea`.

Rank-sim-idea triage (2026-07-09), anchored to shadowfox = 150:

| V | F | R | C | H | Score | Bucket      |
|---|---|---|---|---|-------|-------------|
| 4 | 4 | 3 | 3 | 3 | 432   | worthwhile  |

- V=4: completes the "health economy" half of §5 axis #1; strong §5 alignment
  (deepens a passing canary rather than lighting a dark one).
- F=4: parasites are honest ecological pressure; allogrooming-reduces-a-
  bondmate's-load is emergent care, no director.
- R=3: touches health/mood/illness scoring + the grooming DSE — bounded
  extension, but it touches scoring, so A/B verification required.
- C=3: new slow-decay axis + grooming read-site + illness coupling; ~500–800
  LOC, no GOAP rework.
- H=3: one structural tell fires — a parasite→health→mood→grooming-drive
  feedback loop — but no bespoke canary (grooming silence is already caught) and
  no rare-event cascade. (H-source: balance-grep — grooming/mentor analogues
  burned ~58–60 iterations; adding a coupled metric to that hot surface is a
  moderate, not shadowfox, tax.)

## Approach

Model `ParasiteLoad` as an interoception scalar on the single-axis pattern (raw
perception scalar; personality / neglect compose at the modifier layer, NOT
inside the scalar — see the single-axis-perception-scalars discipline). Grooming
reads it through a consideration on the existing grooming DSE; allogrooming
writes a trim to the *target's* load. Keep the illness coupling's loop gain low
on first ship — the feedback loop is the one shadowfox-adjacent tell, so land the
load axis + grooming read-site FIRST and verify grooming-rate and mortality don't
drift before wiring the illness coupling (staged landing). Every DSE-required
marker ships with its `MarkerSnapshot::set_*` writer in the same commit
(substrate-stub discipline).

## Verification

- Deterministic scenario: a cat isolated from grooming partners on a fixed seed
  accumulates load; a bonded pair keeps each other's load down. Assert
  allogrooming trims the target's load and grooming drive tracks load.
- `just verdict` on a full soak: grooming canary ≥1; hard survival gates
  unchanged after the load-axis-only stage; illness-coupling stage re-verified
  separately for mortality drift.
- Focal-cat trace: grooming DSE shows the load consideration moving its score;
  the bonus pipeline / modifier layer surfaces the neglect composition.

## Log
- 2026-07-09: opened from an `ideonomy` → `rank-sim-idea` generate-then-price
  pass over the §5 sideways-broadening axes. Scored 432 (worthwhile). Land in two
  stages (load axis + grooming read-site, then illness coupling) to contain the
  one feedback-loop tell. Sibling tickets 521 / 523 / 524 from the same pass.
