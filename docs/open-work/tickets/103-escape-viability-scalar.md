---
id: 103
title: escape_viability perception scalar — first-class predicate for adrenaline-valence gates
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

Ticket 047 shipped `AcuteHealthAdrenalineFlee` with a degenerate "always viable" assumption — every injured cat sees the Flee/Sleep lift. The N-valence framework needs a real predicate to differentiate Flee (escape viable) from Fight (escape not viable, combat winnable, ticket 102) from Freeze (neither viable, ticket 105).

A first-class `escape_viability` scalar in `src/systems/interoception.rs` mirrors the 087/088 pattern: perception-derived `[0, 1]` scalar consumed by the modifier layer (and possibly by DSEs directly).

## Design

`escape_viability(cat, ctx) -> f32` composed from:
1. **Terrain openness** — count of walkable tiles within sprint radius (1-2 turns of movement) ÷ total tiles in radius. Closed terrain (walls, dense forest) drops viability.
2. **Threat mobility differential** — own movement speed vs nearest threat's movement speed. Faster than threat → high viability; slower → low viability.
3. **Dependent proximity** — kittens / mate / wounded ally within threat-radius pulls viability down (running abandons them). Bool-style penalty.

Composition: weighted sum normalized to `[0, 1]`; weights configurable via `EscapeViabilityConstants` in `sim_constants.rs`.

Publish via `ctx_scalars` as `"escape_viability"` (mirroring `body_distress_composite`). Marker emission optional — modifiers read the scalar directly per the 088 pattern.

## Scope

- Helper fn in `interoception.rs`.
- `EscapeViabilityConstants` struct in `sim_constants.rs` with terrain_weight, mobility_weight, dependent_penalty defaults.
- Publication via `ctx_scalars` and `ScoringContext`.
- Unit tests: open terrain → high; cornered → low; with kittens → reduced by penalty; faster-than-threat → boosted.
- Integration test: one focal cat in known-position scenarios (wall-corner, open-grass) produces expected viability values.

## Verification

- Synthetic test scenarios construct corners/openings/dependents; assert scalar values land in expected bands.
- Real soak: `just soak 42 && just verdict` — no canary regression (this ticket is perception-only, no behavioral change until ticket 102 reads it).

## Out of scope

- The `combat_winnability` scalar (separate ticket if Fight-branch needs more than escape-viability inversion).
- Marker emission (087 pattern); modifiers can read the scalar directly.
- Hide/Freeze viability (Freeze branch needs both escape-viable=false AND combat-winnable=false; ticket 104 is the DSE, ticket 105 is the modifier branch).

## Log

- 2026-05-01: Opened as substrate dependency for ticket 102 (Fight branch) per 047's narrowing decision.
