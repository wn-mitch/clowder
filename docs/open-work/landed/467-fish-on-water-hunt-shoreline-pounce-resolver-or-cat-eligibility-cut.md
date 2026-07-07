---
id: 467
title: Fish-on-water hunt — shoreline-pounce resolver or cat eligibility cut
status: done
cluster: wildlife
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-05-25
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-07-07
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
- 2026-07-06: promoted from texture question to **step-12 (140)
  landing blocker**. The fluid-movement gait work made cats cycle the
  elect → freeze-at-shore → 10-tick watchdog → re-elect loop ~3×
  faster: fish attempts 1295 → 4071 per soak (93.4% of ALL hunt
  attempts, success 1.4%), collapsing the aggregate hunt-success
  metric 22.3% → 7.8% while land-prey success sat untouched at 98.3%.
  Zapruder trace + full species-split tables in
  `docs/balance/fluid-movement-phase2.md` (step-12 Diagnosis section).
  New mechanical detail: the freeze also fires on LAND — `find_path`
  refuses the impassable Water TARGET, so fish approaches never get
  A* and the greedy fallback strands on concave obstacles (observed:
  Simba pinned beside camp structures 22 tiles from the water).
  Option C's cost is no longer static — it scales with movement
  speed, and 0.4.0 ships fluid movement. A and B both also need a
  fail-fast guard: whichever lands, an unreachable hunt target should
  abandon in ~1 tick, not burn `chase_stuck_ticks` (10) per attempt.
- 2026-07-06: **user chose B (shoreline-pounce)**. Implemented as:
  (1) `pathfinding::hunt_vantage(from, prey, pounce_range, map)` —
  nearest passable tile within the pounce band of the prey; the prey
  tile itself for land prey; `None` for mid-lake fish. (2) Engage
  arms (goap.rs `resolve_engage_prey` + the disposition-chain mirror)
  stalk/approach toward the vantage — a passable target, so A*
  engages and the greedy shoreline/concave-obstacle strand class
  dies. Pounce still gates on Chebyshev dist-to-prey ≤ pounce_range
  and covers the water gap; the kill path was always range-capable.
  (3) Election gate: both target-selection paths (visual DSE
  candidates + scent lock, goap and disposition sides) drop
  candidates with no vantage. (4) Fail-fast: engage aborts
  `Abandoned("prey unreachable (no pounce vantage)")` on tick 1 for
  vantage-less targets (belt-and-suspenders behind the election
  gate), feeding the 073 target cooldown. Verification: 6
  `hunt_vantage` unit tests + `fish_shoreline_pounce` scenario
  (offshore fish killed via bank vantage, mid-lake fish never
  elected — first Fish-species scenario coverage). Found and fixed
  en route: scenario worlds without a Stores structure silently fail
  ALL Hunting/Foraging planning as `GoalUnreachable` (why
  `hunt_acquisition`'s "kills within ~30 ticks" doc had drifted —
  its cat never hunts; my scenario spawns a Stores). Landing gate:
  seed-42 soak — expect fish attempts to collapse toward genuinely
  catchable near-shore elections, aggregate hunt success to become
  the ~land number, survival gates + canaries held.
