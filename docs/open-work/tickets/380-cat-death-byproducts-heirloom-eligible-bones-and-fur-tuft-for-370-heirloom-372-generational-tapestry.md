---
id: 380
title: cat-death byproducts: heirloom-eligible bones and fur-tuft for 370 Heirloom + 372 Generational Tapestry
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: [375]
supersedes: []
related-systems: [crafting.md, the-calling.md, naming.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why
370 (Heirloom recipe) and 372 (Generational Tapestry) are narratively about *cats remembering cats* — multi-generational continuity expressed in objects. Today cat death produces no items. This ticket adds heirloom-eligible byproducts on cat death, routing through the naming-substrate (per `naming.md`) and Calling (per `the-calling.md`) for elder-cat / kin-bonded outcomes specifically. **Not every death yields these.** This is a deliberately scarce, narratively-charged byproduct keyed to: cat-age (elder), cat-bond-density (had kin / mentees / mate), or burial-attendance (death was witnessed and mourned).

## Scope
- Add ItemKind variants: `HeirloomBone`, `FurTuft` (or single `MemorialRelic` variant carrying which cat / which bond — design decision deferred). Carry `creator_entity` field referencing the deceased cat for naming-substrate matching.
- Extend the cat-death resolver (likely `src/systems/death.rs` or in the GOAP step path) to check elder/bond/burial-witnessed conditions and spawn byproducts.
- Emit `MemorialRelicSpawned` message for naming-substrate matcher.
- Cross-reference 370 (Heirloom recipe accepts these as input) and 372 (Generational Tapestry weaves them into multi-cat history).
- Burial canary: 250 demoted burial from the canary set because post-247/248 stability makes deaths rare. This ticket adds production-side, doesn't lift the canary back.

## Out of scope
- Lifting the burial canary — that's a separate decision once death rates rise (or don't).
- Decoration of these items via crafting — that's 370/372.

## Current state
Blocked on 375 (multi-item spawn idiom from prey side is the pattern to reuse for the cat-death side).

## Approach
1. Land 375 first; mirror multi-item-spawn idiom.
2. Decide on single `MemorialRelic` variant carrying metadata, vs distinct `HeirloomBone` + `FurTuft` variants. Lean toward distinct — different recipe consumers (Heirloom wants bone; Tapestry wants tuft) and the naming-substrate can match on either type.
3. Eligibility check: elder-age OR bond-density-above-threshold OR burial-witnessed. Tunable in `sim_constants.rs`.

## Verification
- Scenario: preset elder cat dying with kin present; assert byproducts spawn with correct `creator_entity` linkage.
- `just verdict`: confirm no regression; mythic-texture canary likely boosted by Named Events on relic spawn.

## Log
- 2026-05-16: opened. Plan: `~/.claude/plans/i-d-like-to-do-bright-coral.md`. Follow-on to 375 (cat-side byproduct decomposition for generational continuity).
