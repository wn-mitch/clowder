---
id: 018
title: Ruin clearings (corruption nodes, PMD-flavored)
status: blocked
cluster: null
added: 2026-04-22
parked: null
blocked-by: [016]
supersedes: []
related-systems: [ruin-clearings.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** Third split-out piece. Dungeons-as-corruption-
nodes: uncleared ruins emit corruption radially, the colony
organizes multi-cat clearings (paths → pushback → interior hazards
→ loot). Loot is crafting materials + occasional Named Objects —
**not** gear. Honest-ecology version of "cats go somewhere weird
and come back changed"; complements `the-calling.md`'s interior/
trance version of the same motif.

**Design captured at:** `docs/systems/ruin-clearings.md`
(Aspirational, 2026-04-22).

**Score (scope-cut variant):** V=4 F=4 R=2 C=2 H=2 = **128** —
"earn the slot" (80–300 bucket). The full gear-modifier variant
scores 64 ("defer") and is explicitly rejected. Added as rank 13
in `docs/systems-backlog-ranking.md`.

**Scope discipline (load-bearing — violations drop score to 64):**
1. Loot is crafting material / Named Object only. Never gear.
2. Corruption pushback reuses existing `magic.rs` substrate.
3. Ruin spawn rate is ecological (seasonal corruption + distance
   from hearth), never reactive to colony threat score.
4. Clearing difficulty is environmental, never scaled to colony
   power.

**Dependencies:** hard-gated on A1 IAUS refactor (multi-cat GOAP
coordination on a shared goal) **and** on #16 Phase 1 shipping
(otherwise loot has no consumer). Reuses `magic.rs` corruption
substrate (Built) and `coordination.rs` directives pattern.

**Shadowfox watch:** structurally lighter than shadowfoxes (H=2
vs H=1) because it rides the existing corruption/ward system,
but still bespoke-canary territory. A new `ClearingAttempt`
mortality cause in `logs/events.jsonl` and a
`ruins_cleared_per_sim_year` footer tally are likely required.

**Resume when:** A1 lands and #16 Phase 1 ships. Do not pick up
before both.

## Log

- 2026-04-27: dropped blocked-by 005 — cluster-A umbrella retired; A1 IAUS refactor landed. Still blocked on 016 (crafting items/recipes/stations) for the multi-cat coordination prerequisite.
