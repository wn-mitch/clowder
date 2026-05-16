---
id: 388
title: ColonyKnowledge pre-seeding from generated history
status: blocked
cluster: belief-perception
orchestration: coherent-block
block: worldgen-prehistory
initiative: [generational-continuity, mythic-texture, worldgen-prehistory]
added: 2026-05-16
parked: null
blocked-by: [385, 291]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

009 wants `ColonyKnowledge.entries` non-empty at t=0 — named events like "the Long Winter of year 12" or "the Fox that took Silverpaw" referenceable from sim start. Today `ColonyKnowledge` only accumulates entries during runtime via the carrier-count threshold (or, post-291, via mental-model agreement). The Phase-1 event log generates plenty of name-worthy events; this leg defines the procedure that promotes them into `ColonyKnowledge` at the Phase-1 → Phase-3 boundary. This is **colony-shared substrate** — distinct from #386's per-cat subjective implants.

## Scope

- A boundary procedure (analogous to #386 but writing to `ColonyKnowledge`, not `MentalModel`) that walks the Phase-1 event log and promotes events meeting the named-event criterion (severity, narrative-worth, carrier-count-or-equivalent)
- Composes with 291's mental-model agreement promotion — the same `is_promotable?` predicate runs at boundary insertion as runtime
- Generates the colony's seed-name vocabulary: notable predators, ancestral landmarks, named winters, named foundresses
- ≥1 ColonyKnowledge entry is referenceable by name from t=0 narrative output

## Out of scope

- Per-cat subjective beliefs about the same events (#386 — each cat may know a different version)
- Fate / prophecy that READS these entries (#390)
- Narrative templates that CITE these entries (#391)
- The carrier-count → mental-model agreement migration itself (291)

## Current state

Aspirational — gated on `worldgen-prehistory` block activation (see [9]). Blocked-by [385, 291] — needs Phase-1 event log + 291's promotion-predicate refactor before this leg can encode the procedure cleanly.

## Approach

A boundary system mirroring #386's bulk-insert shape but writing to `ColonyKnowledge`. Reuses 291's `is_promotable?` predicate. The event log is the same Phase-1 artifact #386 reads.

## Verification

- After boundary: `ColonyKnowledge.entries.len() >= 1`
- ≥1 entry is referenceable by name (the named-event criterion)
- Phase-3's "named events per sim year" continuity canary passes from t=0 forward without relying on live-sim event generation in the first sim-week

## Log

- 2026-05-16: opened as leg of `worldgen-prehistory` coherent-block (see [9])
