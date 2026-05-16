---
id: 379
title: ShadowFox banishment byproducts: shadow-bone, fox-pelt, shadow-tooth on successful banishment outcomes
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: [375]
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why
The ShadowFox is Clowder's lore-densest predator — cats drive it off or banish it — yet today it leaves no corpse. 372 (Phase 5 Shrine-Cairn) wants a shadow-relic variant and 377 (rare drops) names `ShadowFangSliver` on successful banishment, but the broader guaranteed-byproduct case (a defeated shadow-fox should leave *something*) has no producer. This ticket follows the 375 prey-byproduct pattern but for the predator side, with corruption-shaped byproducts.

## Scope
- Add ItemKind variants: `ShadowBone` (already exists per current code — wire producer), `FoxPelt` (corrupted-hide variant), `ShadowTooth`. Verify `ShadowBone` consumer chain is updated.
- Extend the banishment outcome path in the ShadowFox combat / defense resolver (likely `src/systems/shadowfox.rs` or similar) to spawn byproducts on successful banishment (distinct outcome from "driven-off" or "fled").
- Cross-reference 372 Shrine-Cairn shadow-relic variant and 377 `ShadowFangSliver` rare drop.
- Mythic-texture canary: each banishment-byproduct spawn emits a Named Event.

## Out of scope
- Rare-tier ShadowFang drop on banishment-quality outcome → 377.
- The combat-substrate ticket that may unify shadow-fox-defense scoring → future / TBD.

## Current state
Blocked on 375 (`engage_prey` pattern is the template; banishment byproduct spawn is the predator-side analog and should follow the same multi-item spawn idiom).

## Approach
1. Land 375 first; mirror its multi-item-spawn idiom in the ShadowFox banishment resolver.
2. Confirm the banishment outcome is distinguishable from other ShadowFox-encounter outcomes in the resolver's witness shape.
3. Emit Named Event on byproduct spawn (mythic-texture continuity-canary anchor).

## Verification
- Scenario: preset cats successfully banishing a ShadowFox; assert byproducts spawn; assert Named Event emitted.
- `just verdict`: confirm mythic-texture canary boosted, no regression on survival gates.

## Log
- 2026-05-16: opened. Plan: `~/.claude/plans/i-d-like-to-do-bright-coral.md`. Follow-on to 375 (predator-side byproduct decomposition).
