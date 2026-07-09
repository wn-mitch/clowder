---
id: 519
title: Emberwing hatch — seasonal ephemeral insect bloom that kittens play-chase, a named once-a-year event
status: ready
cluster: wildlife
initiative: [mythic-texture, world-richness]
orchestration: coherent-block
block: mythic-texture
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

The mythic-texture continuity canary ("≥1 named event per sim year") leans
entirely on the Calling / banishment / visitor / named-object events today —
all colony-internal. The world itself produces no recurring seasonal spectacle
the cats merely witness. The Emberwing hatch adds one: a once-a-year ephemeral
insect bloom that fruits in a specific season, drifts through the map for a
short window, and vanishes. Kittens play-chase it; it is a thing that *happens
to the world* on a calendar the colony doesn't control, which is exactly the
honest-ecology / no-director thesis. It is deliberately the cheapest possible
new creature — it exists to broaden §5 sideways (play) and light a canary, not
to add predation depth.

## Scope

- A seasonal ambient entity (`Emberwing` swarm) that spawns once per sim year
  in its keyed season, wanders as short-lived motes for a bounded window, then
  despawns. No combat, no predation, no den, no life-cycle.
- A play affordance: kittens (and low-need adults) can elect a play-chase
  action against a nearby hatch, scoring on the **existing** play/mood axis —
  no new need, no new scoring channel.
- A named-event emission when the hatch begins, so it registers on the
  mythic-texture canary (one named event per sim year).
- Narrative templates for the hatch (`assets/narrative/play.ron` and/or a new
  `wander`/idle context) so the prose surfaces it.

## Out of scope

- Any predator/prey interaction (nothing eats it, it eats nothing).
- A new need or scoring channel — reuse the play/mood axis only.
- Persistent population dynamics — this is ephemeral by design; no `*_population`
  system, no carrying capacity.

## Current state

New. Not started. Provenance: surfaced by the `ideonomy` skill (seed-42 tuple
`dimension-identification + organon-construction / scale / source·longevity·
hierarchicalness`) as the "ephemeral" gap on the longevity scale — the roster
has no sub-cat-life-span creature — then priced with `rank-sim-idea`.

Rank-sim-idea triage (2026-07-09), anchored to shadowfox = 150:

| V | F | R | C | H | Score | Bucket    |
|---|---|---|---|---|-------|-----------|
| 4 | 5 | 5 | 4 | 5 | 2000  | cheap win |

- V=4: lights mythic-texture (named per-year event) AND play §5 — two hooks.
- F=5: seasons are the sanctioned event generator; a seasonal bloom *is* the thesis.
- R=5: fully isolated ambient spawn + one play affordance; observable; no regression surface.
- C=4: seasonal spawn + play-target affordance, extends existing play/season systems (~300–700 LOC).
- H=5: zero ongoing tax — scores on the existing play/mood axis, no feedback, no canary. (H-source: structural tells — none fired.)

## Approach

Model on the lightest existing spawner rather than the wildlife GOAP stack — no
`WildSpecies`/`PreyKind` entry, no planner. A dedicated ambient system gated by
season fires the spawn, seeds N drifting motes with a short lifespan, and emits
the named-event message. The play affordance is a consideration on the existing
play DSE keyed to hatch-proximity; it must NOT introduce a new need or feed back
into any predator/forage axis (that is what keeps H at 5 — see the shadowfox
contrast in the triage). All tunables (season key, mote count, window length,
play-attraction weight) land in `src/resources/sim_constants.rs` — no inline
magic numbers.

## Verification

- Deterministic scenario: advance to the keyed season on a fixed seed; assert
  the hatch spawns exactly once, emits its named event, motes despawn at window
  end, and ≥1 kitten play-chase fires.
- `just verdict` on a full-year soak: hard gates unchanged (this touches no
  survival axis); mythic-texture canary shows the hatch's named event; play
  canary unaffected or improved.
- No new balance canary should be required — if one is, H was mis-scored and the
  design has leaked feedback into another axis (re-triage).

## Log
- 2026-07-09: opened from an `ideonomy` → `rank-sim-idea` generate-then-price
  pass. Scored the top candidate (2000, cheap win). Sibling ticket 520
  (The Moss-Back) came from the same pass (the "long-lived" gap).
