---
id: 520
title: The Moss-Back — a long-lived landmark beast elders remember and teach kittens about across generations
status: ready
cluster: wildlife
initiative: [generational-continuity, mythic-texture]
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

Every creature in the roster lives on a timescale at or below a cat's lifespan —
prey churn fast, foxes run a full cycle, hawks/snakes are mono-stage adults, the
shadow-fox is coherence-variable. Nothing outlives cat *generations*. That leaves
the generational-continuity canary carried entirely by the cats' own bloodlines
and the generational-knowledge §5 axis with nothing external to anchor on. The
Moss-Back is a single large, slow, near-harmless ancient animal that persists for
many cat generations, keeps a stable home range, and becomes a named landmark:
something an elder cat remembers from kittenhood and can teach kittens about. It
turns "the world has history" from an abstraction into a creature you can walk up
to — the honest-ecology thesis expressed on the longevity axis instead of the
predation axis.

## Scope

- A long-lived, low-count (1–2 map-wide) ambient creature `MossBack` with a
  stable home range, slow wander, and minimal AI (graze / rest / lumber). It does
  not hunt and is not meaningfully huntable (bulk/defense high enough that it is
  a landmark, not prey).
- Longevity that spans multiple cat generations, so the same individual is
  witnessed by successive litters.
- A generational-knowledge hook: presence/sighting of the Moss-Back can seed a
  teachable fact that elders pass to kittens (extends the existing mentoring /
  knowledge-transfer surface — see §7.M kitten-rearing tickets 398/399/450 the
  create-time linker surfaced), and a named-object/landmark registration for the
  mythic-texture canary.
- Narrative templates for encountering / grazing near / a kitten first seeing it.

## Out of scope

- New predation dynamics — the Moss-Back is neither predator nor standard prey.
- A full life-cycle / breeding population — it is deliberately near-static and
  low-count; no `*_population` churn system.
- The generational-knowledge *substrate* itself — if the teachable-fact surface
  does not yet exist at sufficient fidelity, this ticket consumes it minimally or
  is blocked-by the ticket that builds it, rather than expanding it here.

## Current state

New. Not started. Provenance: surfaced by the `ideonomy` skill (seed-42 tuple
`dimension-identification + organon-construction / scale / source·longevity·
hierarchicalness`) as the "longer-than-a-cat-life" gap on the longevity scale,
then priced with `rank-sim-idea`.

Rank-sim-idea triage (2026-07-09), anchored to shadowfox = 150:

| V | F | R | C | H | Score | Bucket     |
|---|---|---|---|---|-------|------------|
| 5 | 4 | 4 | 3 | 4 | 960   | worthwhile |

- V=5: lights generational-continuity + mythic-texture via §5 generational-knowledge-transfer — a being elders remember across kitten generations.
- F=4: a long-lived slow witness-animal fits the honest world; metaphysical weight optional.
- R=4: doesn't hunt, low scoring interaction; the knowledge hook extends the (thin) knowledge-transfer surface — bounded.
- C=3: minimal AI, but the generational-memory hook may lack a surface yet (~700–1.2k LOC; verify before committing).
- H=4: longevity-tracking constant, no fear/ward feedback. (H-source: structural tells — 0–1 fired.)

## Approach

Model as an ambient low-count entity like the lightest wildlife, NOT via the
predator GOAP stack. The load-bearing risk is the generational-knowledge hook:
before building, confirm the teachable-fact / elder-to-kitten transfer surface
exists (tickets 398/399/450 territory). If it does, the Moss-Back seeds a fact
and rides it; if it does not, either scope the hook down to a named-landmark
sighting event only (drops V but keeps the ticket unblocked) or add a `blocked-by`
on the surface ticket. All tunables (count, lifespan-in-generations, home-range
radius, wander cadence) live in `src/resources/sim_constants.rs`.

## Verification

- Multi-generation soak (long `--duration`): the same Moss-Back individual is
  witnessed by ≥2 successive kitten cohorts; an elder-taught fact referencing it
  fires; the landmark registers a named event.
- Deterministic scenario: spawn, stable home-range hold over a long window, a
  kitten's first-sighting narrative fires.
- `just verdict`: survival gates untouched; generational-continuity and
  mythic-texture canaries reflect the hook; no new balance canary required.

## Log
- 2026-07-09: opened from an `ideonomy` → `rank-sim-idea` generate-then-price
  pass. Scored 960 (worthwhile; plan carefully — verify the generational-knowledge
  surface before committing C). Sibling ticket 519 (Emberwing hatch) came from the
  same pass (the "ephemeral" gap on the same scale).
