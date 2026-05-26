---
id: 476
title: Item-effect doctrine: retire cosmetic-only framing, sanction identity-keyed modifier-fetch
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 6440d09e13b0
landed-on: 2026-05-26
---

## Why
The item-effect doctrine drifted into a "cosmetic-only" overcorrection in three
places, and this ticket retires it. `slot-inventory.md` declares wearables
"narrative/identity-only" with "No numeric fields"; ticket 017's guardrail forbids
"numeric capability modifiers"; `crafting.md` rule #2 says "carried-crafted-objects
(tokens, gifts, talismans) stay narrative-only." The intended doctrine was never
"items are cosmetic" — it was **"no boring random stat-sticks; items have stat
*flavor*."** Item effects should be real and mechanical, grounded in what the item
*is* (identity + material + craftsmanship) the way Borderlands manufacturers shape
a gun's character — never random-rolled affixes a la Path of Exile, never decoupled
`+5`-style sliders. `crafting.md` rule #1 already states this correctly; 369's landed
`weapon_class`/`armor_class`/`noise_class`/`durability_tier` classifiers already
*are* the identity→property data layer; 377 (LuckyRabbitsFoot = real fate-escape)
already lives the corrected doctrine. The docs are the lagging artifact. This ticket
aligns the load-bearing pillar + design docs + 017's guardrail with the doctrine
already encoded in code and in the `feedback_no_stat_sticks_not_no_mods` memory.

## Scope
Docs-only. No sim behavior change.
- **Pillar** (`CLAUDE.md` → Design pillars → "Items are real"): revise the mechanism
  sentence. Effects are real, grounded in identity/material/quality, composed by a
  uniform modifier-aggregation layer and fetched-and-applied in resolvers (trace-
  visible) — never random/decoupled stat-stick fields, never `match item.kind`
  effect-logic smeared per resolver. Add 369 to the *Why:* anchors.
- **`docs/systems/crafting.md`** Design constraints: soften rule #1's absolute "no
  generic numeric modifier fields" to "no *random/decoupled* fields; effects derive
  from identity/material classifiers + quality via the aggregation layer" (preserve
  the "+3 vs +5 / spear that snaps" prose). Retire rule #2's carried-objects-are-
  narrative-only carve-out. Re-anchor the "thesis-breaking → re-triggers ranking
  (F→2, H→2, ~96)" trigger to *random stat-sticks / RNG rolls*, not effect-data.
- **`docs/systems/slot-inventory.md`**: drop "narrative/identity-only"; reframe the
  effect list and type guardrail so wearables carry identity + identity-keyed effects
  via the aggregation API (adornment → social/identity effects; functional wearables
  → hunt/combat/stealth). Update the Shadowfox watch mitigation.
- **Ticket 017** (`docs/open-work/tickets/017-*.md`) body prose only: rewrite the
  "Type guardrail (load-bearing invariant)" paragraph + the score-rationale sentence.
  Frontmatter / lifecycle untouched (stays `blocked`, blocked-by [016]).
- **Ticket 016** Design-constraints quick-anchor + ranking parenthetical: match the
  crafting.md rewording. Body prose only (in-progress epic — frontmatter untouched).

## Out of scope
- Building the modifier-aggregation substrate itself — opened as its own ticket
  (369's deferred consumption layer), blocked on 463.
- 017 slot-substrate implementation and 370's adornment producer.
- Re-scoring 017/016 V·F·R·C·H (judgment call left to the maintainer).

## Current state
`crafting.md` rule #1 and 369's classifiers are already correct. The misconception is
localized to the four edits above. The `feedback_no_stat_sticks_not_no_mods` auto-memory
already holds the corrected framing — the docs lagged it. Opened alongside the
modifier-aggregation follow-on ticket in the same session.

## Approach
Pure prose edits. Preserve every "ruined us" precedent and the anti-stat-stick intent;
what changes is *the conclusion* — identity-keyed mechanical effects are sanctioned (and
mandated to flow through a uniform fetch-modifiers API), not forbidden as "cosmetic."

## Verification
- `rg -i 'narrative.only|purely cosmetic|no numeric (capability )?modifier'` over
  `slot-inventory.md`, `crafting.md`, `tickets/017-*.md`, `CLAUDE.md` returns nothing.
- `just check` passes (doc/frontmatter linters; no soak/verdict — docs-only change).

## Log
- 2026-05-26: opened as the doctrine-correction groundwork for the crafting-effects
  line; retires the cosmetic-only overcorrection across pillar + crafting.md +
  slot-inventory.md + 017 guardrail. Companion ticket opened for 369's deferred
  modifier-aggregation consumption layer (blocked on 463).
