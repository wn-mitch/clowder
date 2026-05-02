---
id: 102
title: AcuteHealthAdrenaline Fight branch — cornered/maternal-defense valence
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: null
landed-on: null
---

## Why

047 narrowed the `AcuteHealthAdrenaline` modifier to its **Flee branch**: under acute health-deficit, lift Flee + Sleep so injured cats redirect to escape/recovery. Real fight-or-flight has the opposite valence too — when escape is *not* viable but combat is *winnable* (cornered with kin, defending den at threat-radius, terrain-locked but threat is overmatched), the same adrenaline drives the cat to fight rather than flee. Cornered-cat ferocity, maternal defense.

The 047 ticket text codifies the framework as "N-valence" sharing one adrenaline scalar (`health_deficit`) but gating each branch on a different perception predicate. This ticket ships the Fight valence.

## Scope

- New `AcuteHealthAdrenalineFight` modifier in `src/ai/modifier.rs` reading `health_deficit`. Same smoothstep transition above `acute_health_adrenaline_threshold`.
- Lifts the Fight DSE (proposed `acute_health_adrenaline_fight_lift` ≈ 0.50) AND additively suppresses Flee by the same magnitude (negative lift on Flee), so the cornered cat doesn't see Flee promoted by the Flee branch in the same tick.
- Gated by `escape_viability(cat, ctx) < threshold` — needs ticket 103 to land first as the substrate for this predicate.
- Register in `default_modifier_pipeline` immediately after `AcuteHealthAdrenalineFlee`. Pipeline-count test bumps to 12.
- Unit tests: smoothstep boundary, valence gate (high viability → no Fight lift), interaction with Flee branch (high deficit + low viability → Fight lifted, Flee zeroed/suppressed), gated-boost contract.

## Verification

- Focal-trace soak with a wounded cat next to a den / kittens shows Fight winning the disposition softmax under the new gate, not Flee.
- Hypothesize spec predicting `deaths_by_cause.WildlifeCombat` *increases* slightly (cats who would have fled now fight winnable battles) AND `kitten_protection_event_total` (or equivalent caretake-defense feature) increases. Tradeoff acceptable iff total deaths drop or stay flat.

## Out of scope

- The `escape_viability` perception scalar itself (ticket 103).
- Freeze valence (ticket 105) — different terrain, requires Hide/Freeze DSE first.
- Intraspecies fight-or-flight-or-fawn (ticket 109) — different scalar, different repertoire.

## Log

- 2026-05-01: Opened as one of the §6-valence-framework follow-ons from ticket 047. Blocked-by 103 (escape_viability scalar).
