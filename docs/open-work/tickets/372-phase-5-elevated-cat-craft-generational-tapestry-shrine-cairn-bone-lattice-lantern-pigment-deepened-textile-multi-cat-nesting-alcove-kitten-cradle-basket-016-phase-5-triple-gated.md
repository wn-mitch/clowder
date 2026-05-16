---
id: 372
title: Phase 5 elevated cat-craft — Generational Tapestry, Shrine-Cairn, Bone-Lattice Lantern, Pigment-Deepened Textile, Multi-Cat Nesting Alcove, Kitten-Cradle Basket (016 Phase 5, triple-gated)
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: [371, 366]
supersedes: []
related-systems: [crafting.md, monuments.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Land Phase 5 elevated cat-craft recipes — Generational Tapestry, Shrine-Cairn, Bone-Lattice Lantern, Pigment-Deepened Textile, Multi-Cat Nesting Alcove, Kitten-Cradle Basket — that cats "work up to" as the colony matures. Triple-gated (colony-age ≥3 yr + material scarcity + mastery-arc from 366). Cross-references `monuments.md` for shrine-cairn scope boundary. Explicit not-DF guardrail: collective (multi-cat) or cumulative (multi-season), never individual-rare-strike. Parent epic: [016](016-crafting-items-recipes-stations.md).

## Scope
- Six new `Recipe` entries with the three required gates implemented as `Recipe::is_unlocked(colony, tick) -> bool`.
- Multi-cat contribution tracking on Generational Tapestry, Multi-Cat Nesting Alcove (recorded in crafted-object history field; enforced in code, not prose).
- Multi-season accumulation on Generational Tapestry, Pigment-Deepened Textile (seasonal-pass crafting state).
- Shrine-Cairn cross-registration with `monuments.md` (small shrines here; larger civic / memorial cairns in monuments stub).
- Naming-substrate consumer: each Phase 5 artifact named via `naming.md` event-proximity matcher on the accumulation period (aggregate of events that happened during the craft).
- Wires `RecipeRegistry::is_phase5_unlocked(colony)` (defined in 366) to actual recipe availability.

## Out of scope
- Anything driven by individual mood-strike (→ `the-calling.md` owns that).
- Visible artisan hierarchy in-sim (mastery is latent colony property, not addressed-rank).

## Approach
See `docs/systems/crafting.md` Phase 5 + safeguards against DF-drift. Hypothesis: on a `--duration 1800` (30-min) deep-soak, a seed-42 colony that has crossed year-3 unlocks ≥1 Phase 5 recipe and produces ≥1 Phase 5 artefact; generational-continuity canary holds (kittens-to-adult count unchanged); no Phase 5 artefact produced before year-3 on any controlled seed (gating holds).

## Verification
- `just hypothesize <spec.yaml>` with year-3 unlock vs. controlled-seed not-unlocked treatments.
- `just verdict <run-dir>` — generational-continuity canary holds; no pre-year-3 artifact produced.

## Log
- 2026-05-16: opened as 016 epic decomposition (Phase 5; parent 016, blocked-by 371 + 366).
