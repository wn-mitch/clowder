---
id: 525
title: Lost lore — a skill dies with its last untaught holder (colony loses a craft when the only knower dies with no apprentice)
status: ready
cluster: life-cycle
orchestration: substrate-sensitive
initiative: [generational-continuity, world-richness]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: [collective-memory]
related-balance: []
landed-at: null
landed-on: null
---

## Why

§5 axis #6 (generational knowledge transfer) is built as a *positive* — elders
teach kittens. What makes transmission matter is the anti-phase: the moment an
elder reaches death with a skill un-taught and no apprentice, and the craft is
simply *gone* from the colony until someone re-discovers it. Today knowledge has
no failure mode — a skill known by one cat is as safe as a skill known by all.
Lost lore supplies the honest ecology of knowledge: it can be lost. A colony that
lets its last healer die untaught should feel that loss (a craft it can no longer
perform), and a forgotten craft is exactly the kind of thing that becomes legend —
mythic texture from an ecological fact, no director required.

## Scope

- Per-cat skill/craft *ownership* tracking: a colony "knows" a craft iff ≥1
  living cat holds it. (Reuse the existing skill substrate; this adds the
  colony-level aggregate + the knower set.)
- A loss event on death: when the last living holder of a craft dies, emit a
  `LoreLost` message and remove the craft from the colony's known set. The craft
  becomes re-learnable only from scratch (or from an external source, if one
  exists).
- Mentoring already writes transmission; this ticket adds the *drain* side so the
  known-set can shrink, not only grow.
- Optional second stage: a named-event / narrative emission when a notable craft
  is lost ("the last cat who knew X is gone"), feeding mythic texture.
- Tunables (which crafts are loss-eligible, re-discovery difficulty) in
  `src/resources/sim_constants.rs`.

## Out of scope

- The observational / imitation *acquisition* path — that is ticket 526 (its
  counterpart on the gain side). This ticket is the loss side only.
- A full tech-tree / research system — crafts are the existing skill set, not a
  new progression graph.
- External re-introduction mechanics (a visitor re-teaching a lost craft) — noted
  as a future hook, not built here.

## Current state

New. Not started. Provenance: surfaced by the `ideonomy` skill (2026-07-09 tuple
`negation + substitution / cycle / symmetry·intentionality·materiality`) as the
ANTI-PHASE of the generational life-cycle — the point where knowledge reaches the
death→birth closure and finds no vessel. The `cycle` organon located it; a linear
scale could not. Priced with `rank-sim-idea`.

Rank-sim-idea triage (2026-07-09), anchored to shadowfox = 150:

| V | F | R | C | H | Score | Bucket     |
|---|---|---|---|---|-------|------------|
| 4 | 5 | 3 | 3 | 3 | 540   | worthwhile |

- V=4: makes generational knowledge *matter* by giving it a failure mode; feeds
  generational continuity from the loss side + potential mythic texture (lost-
  craft legend).
- F=5: knowledge genuinely is lost when not passed on; honest, emergent, no
  director — the Watership-Down shape exactly.
- R=3: touches the skill/knowledge substrate + death handling; bounded but
  generational; A/B verify.
- C=3: colony known-set + knower tracking + loss-on-death; reuses
  `collective-memory.md`; ~500–800 LOC.
- H=3: one structural tell (couples death + mentoring, the ~60-iter surface); a
  measured metric (crafts known/lost), not a hard gate, no bespoke canary.
  (H-source: balance-grep — mentoring ≈60 iterations.)

## Approach

Add a colony-level `KnownCrafts` aggregate over the existing per-cat skill
substrate plus the set of living knowers per craft. On a cat's death, decrement;
if a craft's knower set hits zero, emit `LoreLost` and drop it from `KnownCrafts`.
Mentoring is the existing gain path; this is purely the drain. Every message ships
with reader+writer in the same commit (substrate-stub discipline); the `LoreLost`
message needs a live reader (narrative emission and/or the re-learnability gate) —
not a stub. Land the mechanical loss first; the mythic named-event emission is a
clean second stage. Pairs with 526 (emulation) — a colony that can learn by
watching should also be able to forget.

## Verification

- Deterministic scenario: seed a colony with exactly one holder of craft X and no
  apprentice; kill the holder on a fixed seed; assert `LoreLost` fires, X leaves
  `KnownCrafts`, and no cat can perform X afterward.
- Contrast scenario: with an apprentice taught before the holder dies, assert X
  survives — transmission prevents loss.
- `just verdict` on a full soak: generational-continuity canary unaffected or
  enriched; hard survival gates unchanged; if a bespoke canary becomes necessary,
  H was mis-scored (re-triage).

## Log
- 2026-07-09: opened from the `ideonomy` (negation/cycle) → `rank-sim-idea`
  dark-mirror pass over the §5 axes. Scored 540 (worthwhile) — the anti-phase
  standout. Loss-side counterpart to 526 (kitten emulation). Grooming-as-deference
  and rivalry/rejection (both 432) from the same pass were not opened this round.
