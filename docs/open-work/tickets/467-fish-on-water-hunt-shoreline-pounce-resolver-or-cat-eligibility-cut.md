---
id: 467
title: Fish-on-water hunt — shoreline-pounce resolver or cat eligibility cut
status: ready
cluster: wildlife
initiative: [predator-prey-dynamics]
added: 2026-05-25
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

After 465's A*-fallback fix landed, of the remaining 578 "stuck during
approach" hunt events in `logs/tuned-42-59e26d68`, **78.9% are Fish**
(438 events). Cats correctly cannot enter Water tiles (`Water` →
`movement_cost = u32::MAX`); `find_path` correctly returns `None`
when the target itself is impassable
(`src/ai/pathfinding.rs:267-269`). So Fish hunting is structurally
broken at the substrate level — cats commit Hunt, approach, can't
reach the Water tile, stuck-out 10 ticks later. Fish hunt success
post-465 is 3.17% (24 of 757 attempts).

This is a **design question**, not a parameter tune:

- **Option A: retire Fish from cat hunt eligibility.** Cats stop
  selecting Fish as a Hunt target. Removes 757 wasted attempts /
  soak and the 438 stuck events at one stroke. Loses Fish from the
  cat-side food chain.
- **Option B: wire a shoreline-pounce resolver.** Cat approaches to
  shore-adjacent tile, executes a Pounce step that *can* land on
  water (one-tick excursion). Preserves Fish as a food source; adds
  a structural substrate (shoreline detection, water-pounce
  resolver). Bigger change.
- **Option C: leave it.** Fish hunt at 3% success rate is "rare
  treat" texture; the 438 stuck events are visible in the footer but
  not load-bearing.

Decision is unsuited to a parameter sweep; needs a design call.

## Scope

- Decide between A / B / C.
- If A: edit Hunt DSE eligibility filter to drop Fish (or drop Fish
  from prey-species candidate set in score path).
- If B: design + implement the shoreline-pounce resolver. New ticket
  for the substrate work likely.
- If C: close ticket with rationale; document in
  `docs/balance/100-tremor-action-multiplier.md`.

## Out of scope

- Land-prey hunt mechanics — 465 handled the pathing defect.
- Fish as a *foraging* output (a cat fishing at a Fishery building) —
  unrelated to predation eligibility.

## Current state

Blocked on 465 landing.

## Approach

Likely a clarifying conversation with the user to choose A / B / C.
The structural-option menu is short and well-defined; the choice is
values-based (texture vs cleanliness vs effort).

## Verification

- If A: post-fix soak shows 0 Fish hunt attempts.
- If B: post-fix soak shows Fish hunt success ≥ 30%, no new stuck
  surface, survival gates pass.
- If C: no code change; ticket closes with explanation.

## Log

- 2026-05-25: opened from 465's outcome. 438 of 579 remaining stuck
  events are Fish (78.9%). Substrate-correct fix is unclear (A / B / C
  is a design call); blocked on user input.
