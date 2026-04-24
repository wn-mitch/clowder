---
id: 024
title: "§7.W Fulfillment register — MVP container + social_warmth axis"
status: done
cluster: C
added: 2026-04-24
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, warmth-split.md]
related-balance: [warmth-split.md]
landed-at: 47047261
landed-on: 2026-04-24
---

## Why

Ticket 012 (warmth split) phase 3 is blocked on §7.W — the Fulfillment register
specified in `docs/systems/ai-substrate-refactor.md` §7.W.1. No container
component exists for fulfillment axes. Without it, `social_warmth` has nowhere
to live and the warmth conflation (hearth-warmth drowning loneliness) persists.

## Scope

MVP of the §7.W Fulfillment register — the minimum viable container that
unblocks ticket 012 phase 3:

- `Fulfillment` component (`src/components/fulfillment.rs`) with `social_warmth` axis
- Per-tick decay system with isolation-accelerated drain
- Bond-proximity passive restoration
- Scoring-layer integration (`social_warmth_deficit` in `ctx_scalars`)
- Snapshot/event-log emission
- Constants in `SimConstants`
- Spawn-site and schedule registration (3 sites each)
- Unit + system tests

## Out of scope

These are §7.W spec features that land later on top of the MVP container:

- **Sensitization** (per-axis positive-feedback loop) — corruption/compulsion content
- **Tolerance** (diminishing per-unit yield) — pairs with sensitization
- **Source-diversity-modulated decay** — requires multiple axes contributing
- **Mood integration** (§7.W.2 losing-axis deficit → valence drop) — wired in
  mood.rs after container stabilizes
- **Additional axes** (spiritual, mastery, corruption-capture) — each is its own ticket

## Current state

Starting implementation. Phase 2 of ticket 012 (mechanical rename `warmth` →
`temperature`) is already done in uncommitted changes.

## Approach

Flat struct matching the `Needs` pattern — one named field per axis. Restructure
to enum-keyed map when axis count justifies it. See plan file for full
implementation details.

Design spec: `docs/systems/ai-substrate-refactor.md` §7.W.0–§7.W.8.
Warmth-split spec: `docs/systems/warmth-split.md`.

## Verification

- `just check` + `just test` pass
- Seed-42 900s release soak: survival + continuity canaries hold
- `social_warmth` appears in `CatSnapshot` events
- Constants header includes new fulfillment fields

## Log

- 2026-04-24: opened ticket, starting MVP implementation
