---
id: 040
title: Disposition shift after 036 collapsed Courtship / Grooming / Mythic-texture continuity
status: ready
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 036 added the missing Cook branch to the live GOAP disposition router. The structural fix is correct, but it shifted the softmax pool's distribution enough on the seed-42 deep-soak that several unrelated continuity tallies regressed:

| Continuity tally | Pre-036 (a879f43) | Post-036 | Δ |
|---|---|---|---|
| `courtship` | 804 | **0** | wiped |
| `grooming` | 71 | **19** | -73% |
| `mythic-texture` | 48 | **22** | -54% |
| `play` | 111 | 109 | noise |
| `mentoring` | 0 | 0 | unchanged |
| `burial` | 0 | 0 | unchanged |

| Never-fired-expected delta | Pre-036 | Post-036 |
|---|---|---|
| `ItemRetrieved` | silent | **fires** |
| `KittenBorn` | silent | **fires** |
| `GestationAdvanced` | silent | **fires** |
| `KittenFed` | silent | **fires** |
| `BondFormed` | fires | **silent** |
| `CourtshipInteraction` | fires | **silent** |
| `FoodCooked` | silent | silent (tracked in 039) |
| `MatingOccurred`, `MentoredCat`, `GroomedOther` | silent | silent (unchanged) |

Survival canaries hold (Starvation 2 → 0, ShadowFoxAmbush 4 → 5, footer present), so this is a behavioral / continuity drift, not a survival regression — but `courtship 804 → 0` is far outside the ±10% noise band that CLAUDE.md's balance methodology treats as free.

## Suspect cohort

The 036 fix changed the post-softmax `crafting_hint` derivation only — it didn't touch the softmax pool or any other DSE. So the regression's mechanism must be one of:

1. **Routing-side cascade.** Cats whose softmax now resolves to `CraftingHint::Cook` instead of `CraftingHint::Magic` consume different ticks; the change in their movement and target-occupancy reshapes which OTHER cats get to be Mate/Courtship targets, and the feedback loop collapses the courtship loop. Plausible but should be small in magnitude — Courtship 804 → 0 is too dramatic for a second-order effect of a Crafting-tier routing change.
2. **Hidden dependency.** Some downstream system reads disposition or `crafting_hint` and gates a courtship/grooming-adjacent behavior on it. Worth grepping for any consumer of `Disposition` / `CraftingHint` that could be affected by the new Cook value.
3. **A*/zone-cost interaction.** The Cook action set in `src/ai/planner/actions.rs:351` adds zone destinations (Stores → Kitchen) that may inflate path costs on a shared `zone_distances` cache used by Mate/Courtship action plans.
4. **Independent variance.** Single-seed-soak runs of the same commit can drift. Re-run the post-036 soak twice more to bound the noise; if courtship lands ≥100 in either re-run, this regression is partially noise and the magnitude shrinks.

## Investigation steps

1. Run two more seed-42 deep-soaks against the post-036 build, writing to `logs/tuned-42-cook-fix-rerun-{1,2}/`. Compare continuity tallies — bound the noise band.
2. If the regression reproduces, focal-cat trace a cat who was a courtship participant in the pre-036 soak (extract names from `logs/tuned-42-a879f43-pre-cook-fix/narrative.jsonl` filtered to `tier == "Significant"`). What does she pick now?
3. Grep for consumers of `Disposition` / `CraftingHint` that could indirectly gate courtship: `grep -rn 'CraftingHint\|crafting_hint' src/ --include='*.rs'`.
4. If the regression is real and persistent, the right fix is probably to tighten the Cook branch's threshold (currently `cook_score > herbcraft_score && cook_score > magic_score` — strict). Possibilities: route to Cook only if cook_score also exceeds some absolute floor, or only when `food_fraction` is below a threshold (consistent with the Cook DSE's `food_scarcity` axis).

## Concordance prediction

Hypothesis-side: the regression is primarily routing-side cascade and resolves with a magnitude-aware Cook threshold ⇒ Courtship returns to a 100–800 range, Grooming returns to 50+, Mythic-texture returns to 30+. Survival canaries unaffected.

If the regression turns out to be noise (re-run lands courtship ≥ pre-036 value), close this ticket without code changes and update the noise-band documentation in CLAUDE.md.

## Non-goals

- Re-deriving the Cook routing rule from scratch — 036's fix is the structural correct one. This ticket is about *how aggressively* to apply it.
- Investigating the unchanged-silent positives (MatingOccurred, MentoredCat, GroomedOther). Those predate 036 and want their own ticket.

## Pointer

Pre-036 baseline at `logs/tuned-42-a879f43-pre-cook-fix/`. Post-036 soak at `logs/tuned-42/`. The post-036 footer's `continuity_tallies` block is the smoking gun.
