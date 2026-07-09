---
id: 524
title: Colony naming rite — collective ceremony that confers a name on a kitten, place, or event (colony-driven mythic-texture generator)
status: blocked
cluster: magic-mythic
orchestration: substrate-sensitive
initiative: [mythic-texture]
added: 2026-07-09
parked: null
blocked-by: [20]
supersedes: []
related-systems: [naming]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The mythic-texture canary ("≥1 named event per sim year") is fed today by
colony-*internal* individual generators — the Calling's solitary trance,
banishment, visitors, named objects — and, once landed, the seasonal spectacles
(519 emberwing, 520 moss-back). What the world lacks is a *collective, deliberate*
naming act: the colony gathering to confer a name on a new kitten, a place, or a
remembered event. That is the thesis in one ceremony — ecology-with-metaphysical-
weight, no director, meaning made by the cats themselves rather than handed to
them. It adds a second, social path to the mythic-texture canary distinct from
the Calling, and it is the Collective×Abstract-social cell the §5 chart flagged
empty.

## Scope

- A `NamingRite` collective affordance: when a nameable subject exists (a newly
  matured kitten, a significant unnamed place, a landmark event) and ≥N adults
  are co-present, the colony can elect a naming ceremony.
- The rite emits a **named event** (registering on the mythic-texture canary) and
  writes the conferred name through the `NamedLandmark` substrate (ticket 020).
- Participant scoring: co-present adults score a "join the gathering"
  consideration; the rite proceeds when enough join, dissolves if not — no forced
  participation (an honest, refusable directive, not a director cue).
- Narrative templates so the ceremony and the conferred name surface in prose.
- Tunables (quorum N, subject-eligibility window, join weight, per-year cadence
  guard) in `src/resources/sim_constants.rs`.

## Out of scope

- The naming substrate itself — that is ticket 020 (`NamedLandmark`), the hard
  blocker. This ticket consumes it, does not build it.
- Individual naming (the Calling already produces named objects) — this is the
  *collective* path only.
- A governance / voting model — "enough cats join" is co-present scoring, not an
  election.

## Current state

New. Blocked on ticket 020 (NamedLandmark substrate). Provenance: surfaced by the
`ideonomy` skill (2026-07-09 tuple `dimension-identification +
cross-domain-reinstantiation / chart / animacy·autonomy·age`) as the empty
Collective×Abstract-social cell — the MYTHOLOGY re-instantiation of "naming is a
public rite" — the colony-scale ceremony the §5 chart flagged as under-served.
Priced with `rank-sim-idea`.

Rank-sim-idea triage (2026-07-09), anchored to shadowfox = 150:

| V | F | R | C | H | Score | Bucket        |
|---|---|---|---|---|-------|---------------|
| 4 | 5 | 2 | 2 | 3 | 240   | earn the slot |

- V=4: adds a second, colony-driven generator to the (partially-passing)
  mythic-texture canary, distinct from the Calling.
- F=5: this IS the thesis — meaning made by the cats, ecology-with-metaphysical-
  weight, no director.
- R=2: requires multiple cats to score-and-join a shared ritual (coordination) —
  where flippers and second-order effects live; hypothesis-adjacent on whether
  "join the gathering" is reachable with the current substrate.
- C=2: collective coordination + ceremony sequencing; reuses `naming.md` (020)
  but the multi-cat orchestration is 1.2k+ LOC.
- H=3: coordination scoring surface ("join gathering"); bounded, no rare-event
  cascade, no bespoke canary beyond the existing mythic-texture gate.
  (H-source: structural tells — one tell, the coordination scoring surface.)

## Approach

Reuse JointIntention / co-present coordination (tickets 127 / 277) for the
gathering rather than inventing a parallel commitment mechanism — commitment is
one mechanism, not two (design pillar 4). Model the rite as an Activity Intention
with a quorum termination condition; participants elect a "join" consideration
that is refusable (perceivable substrate, not a thumb on the scale). Keep
join-eligibility narrow on first ship (the coordination scoring surface is the
one H tell). On quorum, emit the named event and confer the name via 020. All
tunables in `src/resources/sim_constants.rs`.

Because this is "earn the slot", the balance methodology needs a hypothesis +
prediction before code:
- **Hypothesis:** a colony-scale naming DSE gated on ≥N co-present adults produces
  ≥1 named event per sim year independent of the Calling.
- **Prediction:** in a Calling-suppressed soak, the mythic-texture canary stays
  ≥1, carried by naming rites alone.

## Verification

- Deterministic scenario: assemble ≥N adults with a nameable subject on a fixed
  seed; assert the rite fires, emits its named event, and confers a name through
  020.
- Calling-suppressed soak (`just verdict`): mythic-texture canary ≥1 carried by
  naming rites; hard survival gates unchanged; no flipper in the coordination
  scoring (focal-cat trace shows join/refuse as stable, not oscillating).
- Focal-cat trace: the "join gathering" consideration and its persistence-bonus
  offset are visible in the L2 trace (commitment shown, not hidden).

## Log
- 2026-07-09: opened from an `ideonomy` → `rank-sim-idea` generate-then-price
  pass over the §5 sideways-broadening axes. Scored 240 (earn the slot); blocked
  on 020 (NamedLandmark). Sibling tickets 521 / 522 / 523 from the same pass;
  ◇2 larder/gift economy (96, food-economy coupling) was deliberately NOT
  opened — deferred behind #367 / slot-inventory.
