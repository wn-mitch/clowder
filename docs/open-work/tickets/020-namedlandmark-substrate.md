---
id: 020
title: NamedLandmark substrate (cross-consumer naming)
status: ready
cluster: null
added: 2026-04-22
parked: null
blocked-by: []
supersedes: []
related-systems: [naming.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** Six stubs independently need to produce named
entities that outlive their makers — paths, crafting Phase 3 Named
Objects, crafting Phase 4 decorations, ruin-clearings Phase 3 drops,
the-Calling wards/remedies/totems, monuments. Each rolling its own
name generator produces six inconsistent grammars and six
independent event-proximity matchers. A shared registry + matcher +
event-keyed templates serves all six. Primary lever for the
mythic-texture canary (≥1 named event per sim-year from live-sim
sources, currently ~0).

**Design captured at:** `docs/systems/naming.md` (Aspirational,
2026-04-22).

**Score:** V=2 F=5 R=4 C=4 H=4 = **640** — "worthwhile" scaffolding.
V=2 (no in-world effect until a consumer ships) mirrors the pattern
on `slot-inventory.md`. V rises to effective-4 once one consumer
registers, to effective-5 at three or more.

**Substrate scope:**
- `NamedLandmark` registry resource keyed by `LandmarkId`.
- `match_naming_events` system running after `narrative.rs`.
- Event-kind → template mapping table, extensible per consumer.
- Monument self-naming path (proximity radius 0) as a distinct flow.
- Shared name-spam ceiling: ≤6 named landmarks per sim-year across
  all consumers.

**Scope discipline (load-bearing — keeps H=4):**
1. Shared name-spam guardrail, counted across all consumers.
2. Fallback generator always available per consumer (no hard block).
3. Names carry no numeric modifiers (vocabulary, not stat sheet).
4. Registry is additive; decayed landmarks flagged, never pruned.
5. No player-directed naming.

**Canaries to ship in same PR (4 total):**
1. Named-landmark — ≥1 named landmark per sim-year from live-sim
   events.
2. Name-spam — ≤6 named landmarks per sim-year, aggregated.
3. Consumer-diversity — after all six consumers land, ≥3 distinct
   consumer kinds per sim-year.
4. Fallback-rate — <20% of landmarks use neutral fallback generator.

**Dependencies:** no hard deps. Benefits from one consumer shipping
in the same PR to prove the registration contract. Paths (#19) is
the canonical first consumer because path wear is spatially-
anchored, matching the proximity matcher's strongest shape.

**Shadowfox watch:** minimal — scaffolding with no feedback loop,
no scoring interaction, no mortality surface. Main risk is the OSRS
gravity-well analogue: consumers slipping numeric fields onto
`NamedLandmark` over time. Scope discipline rule 3 is the
type-level guardrail.

**Resume when:** paths (#19) or any other consumer reaches the slot
where the naming substrate becomes load-bearing. Ship as precursor
to a consumer PR (lean) or bundled with first consumer.
