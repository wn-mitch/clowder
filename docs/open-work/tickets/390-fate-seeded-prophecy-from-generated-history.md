---
id: 390
title: Fate-seeded prophecy from generated history
status: blocked
cluster: magic-mythic
orchestration: coherent-block
block: worldgen-prehistory
initiative: [mythic-texture, worldgen-prehistory]
added: 2026-05-16
parked: null
blocked-by: [388]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

009 wants `fate.rs` to carry prophecies/visions rooted in generated history — not just runtime-emerged prophecy. A founder colony that has lived 15 sim-years has its own past; the prophecy substrate should be able to draw on that past as raw material (a vision references a long-dead ancestor; a fated-pair convergence cites a remembered named winter). This leg wires `fate.rs` to read pre-sim `ColonyKnowledge` at t=0 and seed the prophecy queue accordingly.

## Scope

- `fate.rs` reads `ColonyKnowledge` on Phase-3 first tick and seeds the prophecy queue with N entries rooted in pre-sim entries
- Template / vocabulary expansion that lets a prophecy reference a historical named event
- A Calling or fated-pair convergence with an ancestral root is producible from t=0

## Out of scope

- Runtime fate emission (already exists; this leg is the seed)
- The ColonyKnowledge entries themselves (#388 — must exist before this leg can read them)
- The narrative-template work that displays prophecy strings (#391 / existing `narrative.rs`)
- 133 (Calling/destiny vocabulary expansion) — separate vocabulary refactor; this leg uses whatever vocab 133 produces

## Current state

Aspirational — gated on `worldgen-prehistory` block activation (see [9]). Blocked-by [388] (no pre-sim ColonyKnowledge to read otherwise). Pairs with 133.

## Approach

Hook in `fate.rs` startup: read `ColonyKnowledge.entries`, sample N salience-weighted entries, instantiate prophecy-queue entries that reference them. Format: "the [omen] heralds [reference to historical entry]."

## Verification

- ≥1 prophecy queue entry exists at t=0 that references a pre-sim ColonyKnowledge entry
- Mythic-texture continuity canary passes in the first sim-week without relying on live-sim prophecy emission

## Log

- 2026-05-16: opened as leg of `worldgen-prehistory` coherent-block (see [9])
