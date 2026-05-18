---
id: 412
title: Three permanently dead features in activation tracker
status: blocked
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-04-14
parked: null
priority: low
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Current state

### G. Three permanently dead features in activation tracker (low)

- `FoxDenEstablished` — defined in Feature enum, `activation.record()` never called anywhere in code
- `FoxDenDefense` — same
- `CombatResolved` — consequence of bug D; `resolve_combat` never fires

These inflate `features_total` (57) without being able to activate, dragging down the activation ratio. `features_active` at 25/57 is artificially low.

## Log

- 2026-05-18: Renumbered from id `PE-002` → `412` and moved out of `docs/open-work/pre-existing/` into `docs/open-work/tickets/` during Linear migration prep. `pre-existing/` is being retired — the long-lived stragglers are now first-class tickets.
