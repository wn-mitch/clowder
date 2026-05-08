---
id: 231
title: Gate PickupItem / RetrieveRawFood / RetrieveFoodForKitten plans on inventory capacity
status: ready
cluster: null
added: 2026-05-08
parked: null
blocked-by: []
supersedes: [187]
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The post-228 soak (seed 42, commit `bfa6b545`, `logs/tuned-42/`) showed
`inventory full` as the dominant plan-failure reason by an enormous margin —
**21,528 failures across four pickup-class plan templates** in 100,783 ticks:

| Rank | Reason | Count |
|---|---|---|
| 1 | `PickUpItemFromGround: pick_up: inventory full` | 18,374 |
| 4 | `RetrieveRawFood: inventory full` | 2,132 |
| 5 | `GatherHerb: inventory full` | 633 |
| 6 | `RetrieveFoodForKitten: inventory full` | 389 |

These are **planned-then-failed** plans: cats elect a pickup disposition, the
planner generates a plan against `ZoneIs(Stores)` / `ZoneIs(CarcassPile)` /
similar, the cat travels to the target tile, the resolver checks
`inventory.is_full()` at runtime, and the plan fails. Each cycle burns
PlanCreated → travel ticks → PlanStepFailed without producing any food
arriving at any cat. In aggregate the inventory-full cluster is the proximate
upstream signal of the colony's starvation collapse — cats are physically next
to food and cannot pick it up because their inventories are clogged with
something else (build materials, herbs, shiny pebbles, leftover carcass
slots).

Each pickup plan template comments at `src/ai/planner/actions.rs:525-533` that
the chain-entry `CarryingIs(Carrying::Nothing)` veto was intentionally dropped
in ticket 175 — "the runtime resolver gates on `inventory.is_full()`". That
design is failing in practice: the runtime gate prevents the bug (cat doesn't
duplicate items) but doesn't prevent the wasted plan. The substrate already
*has* the truth — the cat's `Inventory` component — but the planner can't see
it, so it generates plans the runtime then has to reject.

**The fix is to surface inventory capacity into the substrate** — author a
marker (`HasInventoryCapacity` / `HasFoodSlot` / similar) from `Inventory`,
gate the pickup plan templates on it at planner-time. Cats with full
inventories should be electing `Discarding` / `Trashing` / `Handing` (already
landed in ticket 178) to make room first, *not* repeatedly trying to pick up
items they can't carry.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 inventory state | `src/components/magic.rs` `Inventory` | `is_full()`, per-slot `Item(kind, modifiers)` enum, `MAX_SLOTS` constant; truth lives here | `[verified-correct]` |
| L1 markers | `src/components/markers.rs` | No `HasInventoryCapacity` / `HasFoodSlot` / `HasMaterialSlot` markers exist; the per-cat capacity signal isn't authored as substrate | `[verified-defect]` (gap) |
| L1 substrate authoring | `src/systems/markers.rs` (or sibling) | No author for capacity markers | `[verified-defect]` (consequence) |
| L2 DSE eligibility | `src/ai/dses/{picking_up,foraging,hunting,herbalism,caretake}.rs` | Pickup-class DSEs score on need + perception; no inventory-capacity gate at the eligibility layer | `[verified-defect]` (no gate) |
| Plan template — PickUp | `src/ai/planner/actions.rs:279-286` `picking_up_actions()` | Single-step `PickUpItemFromGround`; precondition `ZoneIs(CarcassPile)` only | `[verified-defect]` (missing capacity precondition) |
| Plan template — RetrieveRawFood | `src/ai/planner/actions.rs:524-538` | `CarryingIs(Carrying::Nothing)` veto explicitly dropped in ticket 175; precondition `ZoneIs(Stores)` only | `[verified-defect]` (175 over-corrected) |
| Plan template — GatherHerb | `src/ai/planner/actions.rs:397-480` (4 sub-chain variants) | `CarryingIs` veto similarly dropped; preconditions are zone + carrying-target only | `[verified-defect]` |
| Plan template — RetrieveFoodForKitten | `src/ai/planner/actions.rs:600-616` | Comment at 604 explicitly notes "intentionally has no `CarryingIs(Nothing)` precondition" | `[verified-defect]` |
| Resolver — PickUp | `src/steps/building/pickup_material.rs:105-110` | Runtime gate: `if inventory.is_full() { Fail("inventory full") }` | `[verified-correct]` (catches the bug; doesn't prevent the wasted plan) |
| Resolver — RetrieveRawFood / GatherHerb / RetrieveFoodForKitten | `src/steps/...` | Same `inventory.is_full()` runtime check pattern | `[verified-correct]` |
| Disposal counterparty | Tickets 178/176 — `Discarding` / `Trashing` / `Handing` dispositions | Cats CAN dispose of items via these dispositions today, but no marker pressures them to do so when their inventory is full and food is on the ground | `[suspect]` (verify the L2 weight on disposal vs pickup when full) |

## Fix candidates

**Parameter-level options:**
- R1 (planner precondition) — re-add `CarryingIs(Carrying::Nothing)` (or a
  more nuanced typed predicate) to the four pickup plan templates. Reverts
  ticket 175's correction; works for the common case but loses the multi-slot
  flexibility 175 was protecting.
- R2 (DSE eligibility) — add an `inventory_full` eligibility filter to the
  pickup-class DSEs in `src/ai/dses/`. Suppresses the disposition election
  before the planner even runs. Mirrors the existing eligibility-filter
  pattern (`require_alive`, `require_marker`, etc.).

**Structural options:**

- **R3 (split — primary fix; substrate-aligned)** — Author a capacity-marker
  family from `Inventory`, gate plan templates on it.
  - `src/components/markers.rs`: add `HasFreeSlot` / `HasFoodSlot` /
    `HasMaterialSlot` / `HasHerbSlot` markers (per item-class capacity).
    Authored by a new system in `src/systems/markers.rs` from the cat's
    `Inventory` component each tick.
  - `src/ai/planner/actions.rs`: add `StatePredicate::HasMarker(HasFoodSlot::KEY)`
    to `RetrieveRawFood`, `RetrieveFoodForKitten`, and `PickUpItemFromGround`
    (when the target item is food). Add `HasHerbSlot` for `GatherHerb`. Add
    `HasMaterialSlot` for `GatherMaterials` (already covered by ticket 175's
    other path but lint for consistency).
  - `src/ai/dses/picking_up.rs` / `caretake.rs` / `foraging.rs`: optional
    eligibility filter cross-check (substrate marker absence + DSE eligibility
    filter is belt-and-suspenders; keep as a single gate at the planner layer
    if R3 is sufficient).
  - Substrate wiring follows the §4.6 marker discipline:
    `src/components/markers.rs` declares the markers, the authoring system
    inserts/removes them per tick, the planner reads via `HasMarker`. Mirrors
    `MaterialsAvailable` (ticket 096).
  - Lint check: `scripts/check_substrate_stubs.sh` will require the new
    markers to land with at least one reader (planner precondition) and one
    writer (capacity-authoring system) in the same commit, per CLAUDE.md
    "Substrate stubs are forbidden".

- R4 (extend) — Keep the dropped-veto design from 175, but add a
  `slot_available_for_carrying(target_kind)` runtime check at PLAN-CREATION
  time inside the planner. Less aligned with substrate discipline because the
  capacity check then bypasses the marker layer; no lint enforces freshness.
  Listed for completeness; rejected on inspection.

- R5 (rebind) — Map the four pickup actions to a different parent disposition
  (e.g., introduce `DispositionKind::Hauling`). Doesn't address the root
  defect (capacity invisibility); orthogonal to it.

- R6 (retire) — Delete one or more pickup plan templates entirely. Wrong
  shape; pickup is load-bearing for the food chain. Not applicable.

## Recommended direction

**R3 (split)** — substrate-aligned capacity markers + planner gating.
Composes naturally with ticket 178 (`Discarding`/`Trashing`/`Handing` already
land disposal dispositions) — once cats can't elect pickup-class plans with
full inventory, the L2 score for disposal dispositions will rise relatively
and they'll naturally elect to make room before resuming pickup.

R1 is partially subsumed (R3 implements the planner precondition via a marker
gate). R2 is complementary; consider as a follow-on belt-and-suspenders if
the soak still shows residual inventory-full failures after R3.

## Out of scope

- **Disposal-side pressure tuning** — when a cat is full of items and food is
  on the ground, what's the correct relative L2 weight on Discarding vs
  Pickup? Ticket 178 wired the disposal dispositions; the *score-shape* under
  capacity pressure is a balance question (open as follow-on if the soak
  shows cats holding items indefinitely instead of discarding).
- **Multi-slot capacity reasoning** — markers cover "is there at least one
  free slot of type X". A more sophisticated "should I carry 2 of food vs 1
  of herb" decision is a separate, larger ticket.
- **Inventory-disposal preference order** (which item to drop when forced) —
  ticket 178 covers; not in scope here.

## Verification

- Hard gate proxy: `plan_failures_by_reason` post-fix. Target: each of the
  four "inventory full" reasons drops ≥10× from baseline (current: 18,374 /
  2,132 / 633 / 389 ≈ 21k total → target < 2,000 total).
- Indirect proxy on starvation: combined with ticket 230's flee fix, the
  seed-42 soak should hit `deaths_by_cause.Starvation == 0`. This ticket's
  contribution is the inventory-clog half; 230's is the plan-thrash half.
- Microexperiment: extend `tests/scenarios.rs` with a scenario where a cat's
  inventory is preloaded full and food is placed adjacent. Assert: cat does
  NOT generate a `PickUpItemFromGround` / `RetrieveRawFood` plan; cat DOES
  generate a `Discarding` / `Trashing` plan to clear capacity.
- Substrate-stub lint: `scripts/check_substrate_stubs.sh` must pass — every
  new `Has*Slot` marker has a reader (planner precondition via `HasMarker`)
  and a writer (the capacity-authoring system), no allowlist entries.

## Log

- 2026-05-08: opened from `/diagnose-collapse logs/tuned-42` post-228 soak.
  21,528 inventory-full plan failures identified as the dominant plan-failure
  cluster, plausibly upstream of the starvation collapse alongside ticket
  230's plan-thrash root. Plan-template comments at
  `src/ai/planner/actions.rs:525-533` pinpoint ticket 175 as the structural
  precedent that dropped the planner-level capacity gate.
- 2026-05-08: linkages audit via `just similar-linkages --ticket 231`.
  **Confirmed supersede:** ticket 187 (`Kittens starve in the post-184 soak —
  RetrieveFoodForKitten plan-fails dominate`) is the kitten-specific variant
  of the same defect. 187's run showed `RetrieveFoodForKitten: inventory
  full = 2113`, `RetrieveRawFood: inventory full = 1929`, `GatherHerb:
  inventory full = 795`. The current run shows the same shape with the same
  ordering. 231's planner-level capacity gate addresses both adult and
  kitten paths in one fix; 187 folds into 231's verification.

  **Composable / adjacent (not superseded):**
  - `203` — superseded by 230 (plan-thrash root cause); 231 is the inventory
    half of the same colony-starvation collapse, 230 is the flee half.
  - `41` (ready) — Founding haul starvation balance. If 231 + 230 close the
    starvation root cause, the founding-haul balance question becomes
    pure-tuning rather than structural.
  - `93` (in-progress) — Substrate-over-override epic. 231 advances by
    moving capacity-truth from runtime-only to substrate-marker.
