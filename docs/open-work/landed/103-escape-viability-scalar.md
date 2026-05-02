---
id: 103
title: escape_viability perception scalar — first-class predicate for adrenaline-valence gates
status: done
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: 2026-05-02
---

## Why

Ticket 047 shipped `AcuteHealthAdrenalineFlee` with a degenerate "always viable" assumption — every injured cat sees the Flee/Sleep lift. The N-valence framework needs a real predicate to differentiate Flee (escape viable) from Fight (escape not viable, combat winnable, ticket 102) from Freeze (neither viable, ticket 105).

A first-class `escape_viability` scalar in `src/systems/interoception.rs` mirrors the 087/088 pattern: perception-derived `[0, 1]` scalar consumed by the modifier layer (and possibly by DSEs directly).

## Design

`escape_viability(self_pos, nearest_threat, map, has_nearby_dependent, constants) -> f32`. **Single-axis discipline:** pure threat-coupled physics — "given an active threat, can this cat escape?" Ambient closed-space anxiety (claustrophobia / agoraphobia) lives on a separate axis owned by ticket 134's phobia modifier family.

Composed when a threat is present from:

1. **Terrain openness** — fraction of walkable tiles in the `(2 * sprint_radius + 1)²` bounding box centered on the cat. Closed terrain (walls, water, cliff) drops viability.
2. **Dependent penalty** — flat subtractive when the cat is `Parent` (has living kittens) OR holds an active `PairingActivity` pair-bond. Models cost-of-abandonment.

Composition: `(terrain_weight * openness - dependent_weight * dependent_penalty).clamp(0.0, 1.0)`. Defaults `sprint_radius=3`, `terrain_weight=0.7`, `dependent_weight=0.3`, `dependent_penalty=1.0`.

Returns `1.0` when no threat is present (the question is undefined-but-safe; downstream Fight/Freeze gates check threat presence before reading the scalar).

Published via `ScoringContext.escape_viability` and the `"escape_viability"` key in `ctx_scalars`. No DSE / modifier consumes the scalar at landing — tickets 102 / 105 are the consumers.

## What landed

1. **`src/systems/interoception.rs`** — pure helpers `escape_viability(...)` and `count_walkable_tiles_in_box(...)`. Module rustdoc gained an `escape_viability` entry under "Scalars published" naming the single-axis discipline.

2. **`src/resources/sim_constants.rs`** — `EscapeViabilityConstants` struct (`sprint_radius`, `terrain_weight`, `dependent_weight`, `dependent_penalty`) with `serde(default = …)` on every field, `Default` impl, and free-fn defaults. Nested as `pub escape_viability: EscapeViabilityConstants` on `SimConstants` so the full constants block round-trips into the `events.jsonl` header.

3. **`src/ai/scoring.rs`** — `ScoringContext.escape_viability: f32` field. `ctx_scalars` inserts `"escape_viability"` adjacent to `"body_distress_composite"`. All eight test fixtures updated.

4. **`src/systems/disposition.rs` + `src/systems/goap.rs`** — both populator paths compute the scalar via `interoception::escape_viability(...)`. Dependent presence is marker-only in v1: `markers.has(Parent::KEY, entity) || has_pair_bond`, where `has_pair_bond` is derived from a new `Has<PairingActivity>` slot on the existing `mate_eligibility` query (one-tuple-position addition, no new SystemParam).

5. **Tests:**
   - 6 unit tests in `interoception::tests` covering no-threat 1.0, no-threat-short-circuits-terrain, open-terrain high, walled-corner low, dependent penalty, clamp safety.
   - 4 integration tests in `tests/escape_viability_scenarios.rs` covering open meadow, walled corner, open with dependent, no-threat short-circuit.

## Out of scope (parked)

- `combat_winnability` scalar (separate ticket if Fight-branch needs more than escape-viability inversion).
- Marker emission (087 pattern); modifiers can read the scalar directly.
- Hide/Freeze viability (Freeze branch needs both escape-viable=false AND combat-winnable=false; ticket 104 is the DSE, ticket 105 is the modifier branch).
- **Mobility-differential term** — punted because every entity moves 1 tile/tick today. Parked into the **135 continuous-position migration epic**; the term re-enters `escape_viability` in Phase 1 (#138 — per-entity `MovementBudget`).
- **Positional dependent proximity + WoundedAlly axis** — v1 ships marker-only dependent presence. Positional refinement ("dependent within strike radius") and the `WoundedAlly` marker for the third leg of the original spec parked as ticket 136.

## Log

- 2026-05-01: Opened as substrate dependency for ticket 102 (Fight branch) per 047's narrowing decision.
- 2026-05-02: Landed. Single-axis discipline (perception scalars carry one orthogonal axis; trait/personality/ambient anxiety composes at the modifier layer) recorded as a project-wide design principle during planning. Follow-on tickets opened in the same commit:
  - 126 — phobia modifier family (Crusader-Kings-style trait modifiers on urge response).
  - 127 — continuous-position migration **epic** (Vec2<f32> substrate, smooth motion, species speed). Originally drafted as a small "per-species cooldowns" ticket; reframed mid-session into the four-phase epic after the user asked how cooldowns would compose with gridded movement.
  - 128 — WoundedAlly marker + positional dependent proximity (`escape_viability` v1 ships marker-only; this tightens it).
  - 129 / 130 / 131 / 132 — Phase 0 / 1 / 2 / 3 of epic 135. Phase 1 (#138) re-enables `escape_viability`'s mobility-differential term.
