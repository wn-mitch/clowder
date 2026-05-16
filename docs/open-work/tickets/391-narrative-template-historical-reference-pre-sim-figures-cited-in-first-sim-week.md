---
id: 391
title: Narrative-template historical reference (pre-sim figures cited in first sim-week)
status: blocked
cluster: magic-mythic
orchestration: coherent-block
block: worldgen-prehistory
initiative: [mythic-texture, worldgen-prehistory]
added: 2026-05-16
parked: null
blocked-by: [387, 388]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

009's exit criterion includes "≥1 seeded historical event appears in a narrative line in the first sim-week." `narrative.rs` currently emits lines that reference live-sim events; this leg extends the template surface to cite pre-sim figures and named events from t=0 forward. The continuity-canary contract is that the "mythic-texture" canary passes from tick 1, not just after several sim-weeks of live-sim event accumulation.

## Scope

- `narrative.rs` template vocabulary extended with slots for historical figures (from #387 lineage) and historical events (from #388 ColonyKnowledge)
- Template selection logic prefers historical references in the first sim-week (decay weight toward live events over time)
- ≥1 narrative line citing a pre-sim figure or event surfaces in the first sim-week of a fresh Phase-3

## Out of scope

- Lineage substrate itself (#387)
- ColonyKnowledge seeding itself (#388)
- Post-death biography generation (068 — separate scope)
- Prophecy strings (#390 — that's `fate.rs`'s surface, not `narrative.rs`)

## Current state

Aspirational — gated on `worldgen-prehistory` block activation (see [9]). Blocked-by [387, 388] (needs the substrate before templates can cite it).

## Approach

Extend the template enum in `narrative.rs` with historical-reference variants. Template selection weights historical templates higher in the first sim-week (a decay function from t=0).

## Verification

- A fresh Phase-3 produces ≥1 narrative line referencing a historical figure or event in the first sim-week
- Mythic-texture continuity canary holds at t=0 without relying on live-sim event emission

## Log

- 2026-05-16: opened as leg of `worldgen-prehistory` coherent-block (see [9])
