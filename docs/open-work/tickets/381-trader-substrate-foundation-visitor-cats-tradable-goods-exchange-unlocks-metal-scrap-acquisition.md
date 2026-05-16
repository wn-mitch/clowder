---
id: 381
title: trader-substrate foundation: visitor cats + tradable-goods exchange (unlocks metal-scrap acquisition)
status: parked
cluster: world-systems
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: 2026-05-16
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why
Metal is the **first cross-colony economic primitive** in Clowder — a material no cat can produce alone, only acquire from outside. The crafting epic's Phase 3 (370) names metal-set adornment recipes (Bone-and-Wire Tiara, Stone-Set Pin); these can't ship until metal acquisition exists. The user's design call (2026-05-16): metal-scrap is **trader-only**, distinct from prey-byproducts (375) and terrain-harvestables (376). This naturally bootstraps a trader/visitor substrate: visitor cats arrive, carry tradable goods (metal being the first), and exchange via some social/economic substrate.

This is a large foundation ticket on its own — visitor-cat substrate, tradable-goods routing, exchange resolver, arrival/departure cadence, what-they-want-in-return. Parked with stub until ready to design properly. 370's metal-set recipes stay blocked on this.

## Scope (sketch, not finalized)
- Visitor-cat type: an entity that arrives at colony edge, doesn't join the colony, has its own inventory + departure trigger.
- Tradable goods substrate: items that can be exchanged (metal-scrap on traders' side; food / herbs / crafted items on colony side).
- Exchange resolver: cat-to-visitor interaction that swaps inventory contents under some condition (fondness threshold? need-match?).
- Arrival/departure cadence: tunable visitor frequency.
- Naming-substrate hooks for memorable visitor encounters.

## Out of scope (until designed)
- Full trader-economy balance — what colony goods are valuable, what visitors want.
- Trader-routing of *non-metal* goods (textiles, herbs).
- Conflict / theft scenarios at the visitor boundary.

## Current state
Parked. Opened 2026-05-16 as a named follow-on from the input-substrate design thread (plan: `~/.claude/plans/i-d-like-to-do-bright-coral.md`). 370 metal-set recipes (Bone-and-Wire Tiara, Stone-Set Pin) stay blocked on this until designed.

## Approach
Deferred. When ready to design, walk:
1. `just similar` over visitor / trader / exchange concepts to find any latent design in the corpus.
2. Decide visitor-cat substrate before tradable-goods (the entity layer comes first).
3. Author `docs/systems/trader-substrate.md` design doc; this ticket then becomes the implementation glue.

## Verification
TBD when scope is finalized.

## Log
- 2026-05-16: opened as parked. Source: input-substrate design thread (plan: `~/.claude/plans/i-d-like-to-do-bright-coral.md`). Blocker for 370's metal-set recipes.
