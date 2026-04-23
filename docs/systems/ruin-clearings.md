# Ruin Clearings (Corruption Nodes)

## Purpose
Corruption nodes exist as named overworld features; shadowfox ruins are the existing precedent (`src/systems/magic.rs`, `wildlife.rs`). Uncleared ruins emit corruption radially into the tile map — **ecological pressure, not authored quest pacing**. The colony can organize multi-cat clearings: paths to the ruin, staged corruption pushback (reusing `magic::CorruptionPushback`), interior hazards (locally boosted wildlife spawns), and a loot payload on completion. Loot routes to crafting materials (`crafting.md`) and — occasionally — to Named Objects (mythic-texture contribution alongside `the-calling.md`).

This is the PMD-flavored version of "cat goes somewhere weird and comes back changed" — ecological phenomenon, not authored quest. The Calling is the other, interior/trance version of the same motif.

Score (scope-cut variant): **V=4 F=4 R=2 C=2 H=2 = 128** — "earn the slot" per `systems-backlog-ranking.md`. The full gear-modifier variant scores **64** ("defer") and is explicitly rejected; see "Scope discipline" below.

## Thesis alignment
- **Honest ecology.** Ruins aren't authored encounters — they're corruption sources that accumulate pressure unless cleared. Spawn rate is ecological (seasonal corruption + distance-from-hearth), not reactive to colony threat-score. No director.
- **No protagonist shield.** Cats die in ruins. The colony may lose its best hunter on a failed clearing. Stark-framing (`project-vision.md` §3, per 2026-04-22 clarification) applies: named cats matter *and* can die.
- **Sideways-broadening (§5).** Unlocks a colony-scale coordinated action shape (several cats working one goal over multiple days) that existing GOAP doesn't exercise. Feeds the ecological-variety canary by adding a new cooperative mode.
- **Mythic texture.** Named Object drops from completed clearings contribute to the ≥1-per-sim-year canary alongside the Calling.

## Scope discipline (load-bearing — keeps H=2 instead of H=1)
These four disciplines are what separate the scope-cut variant from the shadowfox-class variant. Violating any of them re-triggers ranking:

1. **Loot is crafting material or Named Objects — never gear.** No `DamageReductionMod`, no `HuntingBonusItem`. Named Objects ride the `slot-inventory.md` wearable type, which carries no numeric fields (see that stub's type guardrail).
2. **Corruption pushback reuses existing magic systems.** New substrate is the *Ruin* entity, the *clear* multi-step action, and loot tables. No parallel magic system.
3. **Ruin spawn rate is ecological, not reactive.** Tied to seasonal corruption level + distance from hearth, not to colony threat-score. Reactive spawn rate is the director-shape trap that `raids.md` also has to avoid.
4. **Clearing difficulty is environmental, not scaled.** A hard ruin is hard because it's deep in corrupted territory with hostile wildlife, not because the sim balances to colony power. No dynamic difficulty scaling.

## Initial parameters
| Parameter | Initial Value | Rationale |
|-----------|---------------|-----------|
| Ruin spawn trigger | Season-onset corruption level > 0.6, distance from hearth > 25 tiles | Ecological, not reactive to colony power |
| Clearing stages | 3 | Path-to-ruin → corruption pushback → interior clear |
| Minimum cats for clearing | 2 | Forces coordination; single-cat attempts fail |
| Corruption emission radius (uncleared) | 8 tiles | Real pressure on nearby tiles without over-dominating the map |
| Corruption emission rate (uncleared) | +0.005/tick radial | Slow enough to be planned-for, fast enough to matter |
| Crafting material drop | 1–3 per clearing | Feeds `crafting.md` Phase 1 inputs |
| Named-object drop chance | 15% per clearing | Mythic-texture contribution |
| Cat death risk per stage per cat | ~0.05 | Real mortality, no plot armor |

## Staging
- **Phase 1 — Pressure-and-relief loop.** Ruin entity + passive corruption emission + scope-cut loot (crafting materials only). No interior hazards; clearing is "path to ruin and stand there N ticks with ≥2 cats." Validates the ecological loop before adding mortality pressure. Required hypothesis: *cleared seeds show ~1.5–2× lower background corruption ceiling vs uncleared control.*
- **Phase 2 — Interior hazards.** Wildlife spawns scaled to ruin depth. Real cat mortality during clearings. Required hypothesis: *adult mortality includes a non-zero `ClearingAttempt` cause on seed 42; `ShadowFoxAmbush ≤ 5` canary still holds; `Starvation = 0` still holds.*
- **Phase 3 — Named-object drops.** Crafting-recipe unlocks tied to ruin-specific materials. Optional Calling-inside-a-ruin hook producing a named wearable. Required hypothesis: *mythic-texture named-event count per sim year rises ≥1 independent of Calling trigger rate.*

## Dependencies
- **Hard-gated on A1 IAUS refactor** (GOAP multi-cat coordination on a shared goal).
- **`crafting.md` Phase 1 must ship first** (otherwise loot has no consumer and the crafting-material drop is inert).
- Reuses `magic.rs` corruption substrate (already Built).
- Benefits from `coordination.rs` build-pressure directives (provides the "send a clearing party" directive shape).
- Interacts with `slot-inventory.md` Phase 3 for Named Object loot routing (soft-dep; Phase 1 and 2 don't need it).
- **NamedLandmark substrate.** Phase 3 Named-object drops are one of six convergent consumers of the shared naming substrate documented in `naming.md` (registry + event-proximity matcher + event-keyed name templates). Ruin-clearing kills and Calling-inside-a-ruin events become naming triggers under the shared matcher (e.g. "{cat}'s Ruin-Walk" derived from a clearing-death event), rather than Phase 3 rolling its own name generator. Consumers: `paths.md`, `crafting.md` Phase 3, `crafting.md` Phase 4, `ruin-clearings.md` Phase 3 (this file), `the-calling.md`, `monuments.md`.

## Shadowfox comparison
Structurally lighter than shadowfoxes despite being in the same ecological-threat family:
- **F=4 vs shadowfox F=5** — slightly lower only because a dungeon-clearing loop has a plausible misbuild into authored-encounter pacing if disciplines 1–4 above slip.
- **H=2 vs shadowfox H=1** — decisively better because the dungeon-existence mechanic rides the existing corruption/ward system rather than inventing a new predator feedback loop.
- **Still bespoke-canary territory.** A new `ClearingAttempt` mortality cause in `logs/events.jsonl` is likely required; a `ruins_cleared_per_sim_year` footer tally helps surface Phase-1 dormancy.

## Tuning Notes
_Record observations and adjustments here during iteration._
