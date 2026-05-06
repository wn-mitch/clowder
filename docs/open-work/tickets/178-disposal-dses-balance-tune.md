---
id: 178
title: Balance-tune disposal DSEs from default-zero (176 follow-on)
status: ready
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 176 landed four inventory-disposal DSEs (`Discarding`,
`Trashing`, `Handing`, `PickingUp`) that ship dormant — each
uses a single `Linear { slope: 0.0, intercept: 0.0 }`
consideration so the score collapses to 0.0 regardless of input.
The cats can't elect any of these dispositions today.

This ticket replaces the zero-curves with real considerations
that drive the Maslow-tier-1 disposal behavior the parent ticket
calls for:

- **Discarding** — score lifts when the cat's inventory is
  overstuffed AND no Midden is reachable AND `ColonyStoresChronicallyFull`
  is set. Drop on the ground for foragers / scavengers.
- **Trashing** — score lifts when the cat's inventory is
  overstuffed AND a Midden is reachable. Walks to the midden
  and deposits.
- **Handing** — score lifts when the cat's inventory is
  overstuffed AND a target cat needs the item (kitten, hungry
  elder, requesting mate). Needs a target-taking sibling DSE
  to pick the recipient.
- **PickingUp** — score lifts when the cat has inventory room
  AND a desired ground item is in range. Load-bearing for the
  kill→carcass-on-ground→pick-up flow.

## Direction

- Add `inventory_overstuffed` scalar to `ctx_scalars` (high
  when cat's inventory holds food past their per-tick
  consumption budget AND Stores is chronically full).
- Replace each DSE's zero-curve with a real consideration:
  Discarding/Trashing/Handing keyed on overstuffed-ness;
  PickingUp keyed on ground-item proximity + inventory room.
- Tune via the four-artifact methodology (`just hypothesize`):
  predict that elections lift Discarding/Trashing in the
  overflow-collapse seed; observe the post-fix
  `food_available=true` ratio recovers to ≥90%.
- Open a target-taking DSE for `Handing` recipient selection.

## Out of scope

- Drop / Trash / Handoff dispatch wiring (177).
- ColonyStoresChronicallyFull → Build DSE wiring (179).

## Verification

- Post-fix soak's Wren `food_available=true` ratio ≥ 90% (per
  ticket 176 §Verification).
- Disposal Features (`ItemDropped` / `ItemTrashed` /
  `ItemHandedOff`) fire ≥ 1 per soak; promote to
  `expected_to_fire_per_soak() => true` if reliable.
- `OverflowToGround` count drops below baseline as disposal
  intercepts overflow before resolvers spawn ground-items.
- Survival hard-gates pass.
- `colony_score.aggregate` returns within ±10% of pre-175
  baseline (2175).

## Log

- 2026-05-05: opened by ticket 176's closeout. Disposal DSEs
  shipped default-zero in stage 3; this ticket lifts them.
