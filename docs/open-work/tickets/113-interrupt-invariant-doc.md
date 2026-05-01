---
id: 113
title: Document the interrupt invariant + lurch-vs-pressure modifier doctrine
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Two doctrines surfaced during ticket 047's work that deserve doc-stub status to prevent re-discovery:

1. **Interrupt invariant** (047's "## Family resemblance" section): an interrupt that fires every tick must produce a state change that prevents it firing next tick — otherwise it's a no-op cycle while real damage accumulates. Codifying as a maintained invariant catches the next instance of the family-of-bugs (042, 043, 047 are case-law).

2. **Lurch vs pressure modifier shapes** (047's design discussion): acute distress (adrenaline, fight-or-flight, surprise) lands on the IAUS as a *lurch* — sigmoid step at threshold, large magnitude, possibly with valence-split context-gates. Sustained pressure (hunger gradually building, energy slowly draining) lands as a *ramp* — graded linear lift, moderate magnitude, single-direction targeting. Picking the curve picks the semantic model; two doc artifacts capture this.

## Scope

- Write `docs/systems/interrupt-invariant.md` — one-page stub naming the invariant, listing 042/043/047 as case-law, with a "before adding a new interrupt branch, check this doc" preamble.
- Write `docs/systems/distress-modifiers.md` — codifies the lurch-vs-pressure distinction. Maps each modifier shipping under tickets 047/088/106/107/108/110 to its category. Notes the perception-richness pattern (more distress kinds = more modifiers, not bigger lift on one).
- Reference both from `docs/systems/ai-substrate-refactor.md` (the §3.5.1 modifier-pipeline section) so the doctrine is discoverable from the canonical substrate spec.

## Out of scope

- Writing the modifiers themselves — those are tickets 102/106/107/108/110.
- Updating CLAUDE.md to reference the new docs (CLAUDE.md is a navigation index, not a doctrine sink; the docs themselves carry the load).

## Log

- 2026-05-01: Opened as doctrine follow-on from ticket 047. The 047 ticket text explicitly anticipated this work ("worth a doc note in `docs/systems/`").
