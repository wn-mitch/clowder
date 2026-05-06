---
id: 187
title: Kittens starve in the post-184 soak — RetrieveFoodForKitten plan-fails dominate
status: ready
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The post-184 seed-42 deep-soak (commit `4db67313`) is the
first run on this branch where the mating pipeline actually
fires (`MatingOccurred = 2`, three kittens born). Two of those
three kittens starve to death:

- `tick:1286050:Death:Pebblekit-83` (Starvation)
- `tick:1290689:Death:Ivykit-10` (Starvation)

`deaths_by_cause.Starvation = 2` is a hard-gate failure
(canonical hard gate: `Starvation == 0`). The third kitten is
alive at end-of-run but `KittenMatured = 0`, so generational
continuity isn't established yet on this seed/duration.

The `plan_failures_by_reason` distribution implicates the
caretaking pathway:

```
2113× RetrieveFoodForKitten: inventory full
1929× RetrieveRawFood: inventory full
 795× GatherHerb: inventory full
```

`RetrieveFoodForKitten` failing 2113 times with reason
"inventory full" is the load-bearing signal. When a parent
elects Caretake → goes to Stores → tries to retrieve food for
their kitten, the resolver finds the parent's inventory full
(probably leftover prey from a prior Hunt that hasn't been
deposited yet, or herb backup) and fails the step rather than
deferring or substituting. The kittens get visited but not
fed.

For context: pre-184 soaks never reached this code path
(MatingOccurred = 0), so this defect was masked by the
upstream over-gating. The 184 fix surfaced it.

## Current architecture (layer-walk audit, preliminary)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs::IsParentOfHungryKitten` | Per-cat marker fires on parents whose kitten's hunger drops below threshold; gates Caretake DSE eligibility | `[suspect]` (verify the marker authors correctly post-184) |
| L2 DSE scores | `src/ai/dses/caretake.rs` (or equivalent) | Caretake elects when parent has hungry kitten + parent has capacity | `[suspect]` (does Caretake's eligibility check inventory-not-full as a precondition? It should — running RetrieveFoodForKitten with full inventory is wasted work) |
| Plan template | `src/ai/planner/actions.rs` (caretaking_actions) | Plan: TravelTo(Stores) → RetrieveFoodForKitten → TravelTo(Kitten) → FeedKitten | `[suspect]` (where exactly does the inventory-full check live? Resolver-level or planner-level?) |
| Resolver | `src/steps/disposition/feed_kitten.rs` (or sibling) | The "inventory full" rejection signal needs a layer-walk to identify whether it's silently failing or genuinely blocked | `[suspect]` |
| Marker | `IsParentOfHungryKitten` removal/persistence | If the parent dropped this marker before re-attempting Caretake, the plan re-elects something else and never returns | `[suspect]` |

## Diagnostic gaps

1. **Which inventory was full** — the parent's? Did they have
   prey/herbs from prior plans that they should have deposited
   first? `/logq trace logs/tuned-42 --cat=<parent-of-Pebblekit>`
   for the moments preceding the kitten's death.
2. **Was Caretake elected** before the starvation, or did the
   parent never elect Caretake at all? `/logq cat-timeline
   logs/tuned-42 <parent-name> --tick-range=<near-death>` —
   confirm.
3. **Is FeedKitten ever firing?** `KittenFed = 5` in the
   post-184 soak's SystemActivation — so it does fire, just
   not enough to keep up with hunger drain. Compute fed-per-day
   per-kitten vs the kitten's hunger drain rate.
4. **Is the kitten's `IsHungryKitten` marker behaving?** The
   parent's `IsParentOfHungryKitten` is downstream of the
   kitten's marker; both must persist long enough for the
   plan to complete.

## Direction

Per CLAUDE.md bugfix discipline, structural-revision menu before
parameter tuning:

- **split** — give RetrieveFoodForKitten its own resolver shape
  separate from the umbrella Caretake disposition. If the
  parent has full inventory, the resolver could deposit first
  rather than fail the plan outright.
- **extend** — keep Caretake's disposition, but add an
  inventory-not-full eligibility check at L2 so cats with full
  inventories aren't elected for the kitten-feeding plan in the
  first place. They'll re-elect when inventory clears.
- **rebind** — chain a Drop / Deposit step into the front of
  the caretaking plan template if the parent's inventory is
  occupied with non-food items.
- **retire** — N/A; kitten-feeding is load-bearing.

## Out of scope

- Generational continuity (KittenMatured) — directly downstream
  of kitten survival; if kittens stop starving they'll mature.
  Don't separately balance KittenMatured until 187 lands.
- Burial canary (still 0 in post-184) — separate ticket if it
  persists across longer soaks, or rolls into a life-stage
  coverage ticket.
- Kitten food-value rebalance — last resort if the structural
  fix isn't enough.

## Verification

- New scenario: one parent with full inventory + one hungry
  kitten + one Stores. Expected behavior: parent deposits
  inventory, then retrieves kitten food, then feeds. Currently
  fails the deposit prerequisite.
- Soak gates: post-fix seed-42 deep-soak at the same SHA must
  show `deaths_by_cause.Starvation == 0` (or specifically: zero
  kitten starvations) AND `KittenFed` count rises from 5 to a
  level proportionate to kitten count × duration.
- `/logq trace` on a parent-kitten pair confirms the
  RetrieveFoodForKitten plan completes end-to-end.

## Log

- 2026-05-06: opened from ticket 184's post-fix soak. The 184
  fix unlocked the mating pipeline; this is the first defect
  the active pipeline surfaces. Three kittens born, two
  starved (`Pebblekit-83`, `Ivykit-10`). `RetrieveFoodForKitten:
  inventory full` plan-fails dominate (2113×) — load-bearing
  signal but layer-walk needed to attribute structurally.
