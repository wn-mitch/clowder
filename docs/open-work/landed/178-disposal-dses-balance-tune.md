---
id: 178
title: Balance-tune disposal DSEs from default-zero (176 follow-on)
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: c2317dc8
landed-on: 2026-05-06
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
- **Handing curve lift + recipient picker** — 188.
- **PickingUp curve lift + HasGroundCarcass marker** — 185.

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
- 2026-05-06: in-progress. Scope narrowed to **Discarding +
  Trashing** only; Handing curve lift defers to 188 (target
  picker), PickingUp curve lift defers to 185 (HasGroundCarcass
  + multi-axis composition). Trashing eligibility gains a new
  `HasMidden` colony-singleton marker (parallel to
  `HasFunctionalKitchen`/`HasStoredFood`). Plan at
  `~/.claude/plans/prepare-to-work-ticket-logical-wave.md`.
- 2026-05-06: landed. Substrate complete; behaviour ships
  conservative.

## Resolution

The structural substrate landed; the headline `food_available`
gate was *not* met (see follow-on §Surfaced regression).

**Substrate shipped:**
- `inventory_excess` per-cat scalar (`src/ai/scoring.rs`),
  fed by a new `inventory_food_fraction` field on `ScoringContext`
  populated from `Inventory::food_count()`.
- Discarding curve `Logistic(slope=8, midpoint=0.5)` on
  `inventory_excess`; eligibility tightened to
  `forbid(Incapacitated) ∧ require(ColonyStoresChronicallyFull)`.
- Trashing curve same shape; eligibility
  `forbid(Incapacitated) ∧ require(ColonyStoresChronicallyFull) ∧ require(HasMidden)`.
  Per the user's directive: *cats should not trash food unless
  Stores is chronically full*. The `HasMidden` colony-singleton
  marker (declared in `markers.rs`, authored by
  `update_colony_building_markers`, threaded through
  `colony_state_query` → `MarkerSnapshot`) differentiates the
  Trashing route from the Discarding fallback.
- TrashItemAtMidden dispatch resolves the nearest Midden from
  `snaps.midden_entities` when `target_entity` is None — mirrors
  the `DepositFood` / `EatAtStores` fallback pattern.
- All four disposal DSEs (Discarding / Trashing / Handing /
  PickingUp) now uniformly dispatched in `score_actions`. Per
  substrate-over-override doctrine: Handing and PickingUp stay
  dormant via eligibility filters that require markers
  (`HasHandoffRecipient`, `HasGroundCarcass`) that 188 / 185 will
  author — allowlisted in `scripts/substrate_stubs.allowlist`
  until those tickets land.
- New tuning constants
  `disposal_inventory_excess_{slope,midpoint}` in
  `SimConstants::scoring`.
- New scenarios under `src/scenarios/disposal_election.rs`
  exercising election (Discarding / Trashing / idle baseline /
  no-marker rejection); existing `disposal_dispatch.rs`
  hand-stamped tests still pass.

**Headline gate not met.** Wren `food_available=true` ratio
came out at **21.9%** vs. the ≥90% target. Disposal Features
(`ItemDropped` / `ItemTrashed` / `ItemHandedOff`) all fired 0 ×
across the soak — `ColonyStoresChronicallyFull` never latched
because `DepositRejected` stayed at 0 (Stores never overflowed).
That's *correct conservative behaviour* — the disposal pipeline
is dormant when the colony has room — but it means 178 by itself
doesn't recover the post-176 collapse. The 4.3% → 21.9% lift
came from the Trashing dispatch fallback unblocking some chain
failures, not from disposal DSEs winning L3.

## Surfaced regression

Comparing this soak against `tuned-42-pre-178` (commit
`4db67313`, the 184 fix immediately before 177 + 178):

| Metric | pre-178 | post-178 | Δ |
|---|---|---|---|
| `seasons_survived` | 5 | 2 | -60% |
| `colony_score.aggregate` | 1830 | 1091 | -40% |
| Wren `food_available=true` | 71% | 22% | -69% |
| `nourishment` | 0.473 | 0.516 | +9% |
| `deaths_starvation` | 2 | 2 | = |
| `WildlifeCombat` deaths | 1 | 2 | +1 |

The disposal Features stayed at 0, so the regression isn't from
cats trashing food they should eat. Suspects: **Bevy schedule
perturbation** from adding `Has<HasMidden>` to `colony_state_query`
(memory: "a new system's SystemParam tuple defines conflict
edges; even an empty body re-orders siblings"); the four extra
`score_dse_by_id` calls running modifier pipelines on default-
zero DSEs even when ineligible (gated, but the L2 trace path
runs). Opened as ticket **189** for layer-walk diagnosis.

## Land-day follow-ons

- **188** — unparked (`status: ready`); Handing target picker
  is now load-bearing because the Handing DSE eligibility
  filter requires `HasHandoffRecipient`, which 188 must author.
- **189** — *new*: post-178 `food_available` regression
  layer-walk. Compare focal traces between `tuned-42-pre-178`
  and the post-178 archives; identify whether the perturbation
  is at the Bevy-schedule layer or the L2 modifier-pipeline
  layer.
