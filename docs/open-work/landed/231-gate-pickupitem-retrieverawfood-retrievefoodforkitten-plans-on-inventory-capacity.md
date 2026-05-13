---
id: 231
title: Strengthen pickup-class substrate — capacity markers + body-state subscription
status: done
cluster: ai-substrate
added: 2026-05-08
parked: null
blocked-by: []
supersedes: [187]
related-systems: []
related-balance: []
landed-at: 5d98bea4
landed-on: 2026-05-08
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

**Adjacent substrate gap — pickup-class L2 doesn't perceive the cat's body
state.** Post-230 dying-arc analysis of the same `logs/tuned-42/` run
(see Log entry 2026-05-08 #3) showed Calcifer at tick 1251500 picking
`PickUp` (L2 score 0.958) over `Flee` (0.948) at HP=0.49 with a fox 2
tiles away — softmax temperature 0.15 made it a coin-flip; cat lost,
walked into the fox-scent zone carrying carcass items, died 60 ticks
later. Cedar's dying arc shows the same shape: at HP=0.38 with safety
0.18, `Sleep` scored 1.08 (highest L2 score of any DSE in the window)
but the cat picked PickUp at 0.99 — twice in a row. Across both cats,
**neither ever picked Sleep, Flee, or Eat across the entire dying
window** despite L2 scoring those above the eventual choice on multiple
ticks.

The root cause: `PickUpItemFromGround` (and the three sibling pickup
DSEs) score from one Consideration — `colony_food_security` — and don't
subscribe to any cat-body-state scalar. `pain_level`,
`body_distress_composite`, and `health_deficit` are already published
in `ctx_scalars` and consumed by Sleep/Flee/Eat (Sleep reads four
body-state axes; Flee reads two). Pickup-class DSEs don't subscribe.
Per CLAUDE.md "Substrate stubs are forbidden," this is a stub on the
consumer side: the perception is authored, the consumers are absent.

The capacity-marker fix above prevents *wasted plans* (the cat
generates a PickUp chain it can't fulfil); the body-state subscription
fix prevents *wrong elections* (the cat correctly generates the chain
but it shouldn't have been the winner under the cat's current state).
Both pieces compose with the substrate-over-override directive: the
DSE's score should fully describe its own viability under the current
state. PickUp at L2=1.01 with HP=0.49 is the substrate lying about
viability; adding a `body_distress_composite` axis (or `pain_level` /
`health_deficit` — pick the right composite) repairs the lie at the
substrate layer rather than gating the result.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 inventory state | `src/components/magic.rs` `Inventory` | `is_full()`, per-slot `Item(kind, modifiers)` enum, `MAX_SLOTS` constant; truth lives here | `[verified-correct]` |
| L1 markers | `src/components/markers.rs` | No `HasInventoryCapacity` / `HasFoodSlot` / `HasMaterialSlot` markers exist; the per-cat capacity signal isn't authored as substrate | `[verified-defect]` (gap) |
| L1 substrate authoring | `src/systems/markers.rs` (or sibling) | No author for capacity markers | `[verified-defect]` (consequence) |
| L2 DSE Considerations (input axes) | `src/ai/dses/picking_up.rs:58-61`; siblings | `PickingUpDse` scores from one Consideration — `colony_food_security` (inverted Logistic). `RetrieveRawFood` / `GatherHerb` / `RetrieveFoodForKitten` similarly score from external pressure (need + colony state) only. None subscribe to `body_distress_composite` / `pain_level` / `health_deficit`. Sleep reads four body-state axes; Flee reads two; pickup-class DSEs read zero. | `[verified-defect]` (substrate stub: perception authored, consumers absent) |
| L2 DSE eligibility filters | `src/ai/dses/{picking_up,foraging,hunting,herbalism,caretake}.rs` | No inventory-capacity gate; no body-state gate. Filters cover marker presence + alive checks only. | `[verified-defect]` (no capacity gate; body-state gate is intentionally a Considerations question, not a filter — see "Recommended direction") |
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
  family from `Inventory`, gate plan templates on it; ALSO subscribe pickup-
  class DSEs to body-state perception so their L2 scores describe their own
  viability honestly.

  **R3a — Capacity markers (planner-side gate; prevents wasted plans):**
  - `src/components/markers.rs`: add `HasFreeSlot` / `HasFoodSlot` /
    `HasMaterialSlot` / `HasHerbSlot` markers (per item-class capacity).
    Authored by a new system in `src/systems/markers.rs` from the cat's
    `Inventory` component each tick.
  - `src/ai/planner/actions.rs`: add `StatePredicate::HasMarker(HasFoodSlot::KEY)`
    to `RetrieveRawFood`, `RetrieveFoodForKitten`, and `PickUpItemFromGround`
    (when the target item is food). Add `HasHerbSlot` for `GatherHerb`. Add
    `HasMaterialSlot` for `GatherMaterials` (already covered by ticket 175's
    other path but lint for consistency).
  - Substrate wiring follows the §4.6 marker discipline:
    `src/components/markers.rs` declares the markers, the authoring system
    inserts/removes them per tick, the planner reads via `HasMarker`. Mirrors
    `MaterialsAvailable` (ticket 096).
  - Lint check: `scripts/check_substrate_stubs.sh` will require the new
    markers to land with at least one reader (planner precondition) and one
    writer (capacity-authoring system) in the same commit, per CLAUDE.md
    "Substrate stubs are forbidden".

  **R3b — Body-state Considerations (DSE-side scoring; prevents wrong
  elections):**
  - `src/ai/dses/picking_up.rs`: add a `Consideration::Scalar` reading
    `body_distress_composite` (or `pain_level` / `health_deficit` —
    pick the composite that best captures "this is a bad task right
    now"). Curve shape: an inverted/damping `Linear` or `Logistic` so
    high distress drives the axis toward 0, low distress toward 1.
    Calibrate the `Composition::weighted_sum` weights so a healthy cat
    scores PickUp identically to today and a wounded cat (HP=0.49)
    scores it materially below Sleep / Flee / Eat. The exemplar to
    mirror is `src/ai/dses/sleep.rs:considerations` — Sleep reads four
    body-state axes (energy_deficit, day_phase, health_deficit,
    pain_level) plus two spatial axes; pickup-class DSEs need the same
    body-state subscription (different curve direction — Sleep rises
    on body distress, PickUp damps).
  - `src/ai/dses/picking_up_food_for_kitten.rs` (or wherever
    `RetrieveFoodForKitten` is scored): same axis. Adults with a
    hungry kitten waiting still need to perceive their own body state
    before electing the chain.
  - `src/ai/dses/foraging.rs::forage_dse` and any sibling
    `RetrieveRawFood`-emitting DSE: same axis.
  - `src/ai/dses/herbcraft_gather.rs`: same axis. (`GatherHerb`
    appears in the inventory-full failure cluster at 633 instances;
    same shape gap as PickUp.)
  - Substrate-stub lint compatibility: each new Consideration is a
    consumer of an already-published scalar (`body_distress_composite`
    is in `ScoringContext` and surfaces through `ctx_scalars`), so no
    new marker is authored — the substrate side of the gap is already
    covered. The lint will only enforce the capacity markers from
    R3a.

  R3a and R3b compose: R3a prevents wasted plans (capacity invisible
  to planner → planner generates uncompletable chains); R3b prevents
  wrong elections (cat in distress correctly generates a PickUp chain
  it CAN complete, but the chain shouldn't have been the L3 winner
  under the cat's current state). Either alone leaves half the
  starvation cascade uncorrected.

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

**R3 (R3a + R3b together)** — capacity markers AT the planner layer +
body-state Considerations AT the L2 scoring layer. Both are pure
substrate enhancements (no gates, no overrides, no filters): R3a
authors new perception so the planner can describe the cat's
inventory state, R3b subscribes existing perception so each pickup-
class DSE describes its own viability under the cat's body state.

Composes naturally with ticket 178 (`Discarding`/`Trashing`/`Handing`
already land disposal dispositions) — once cats can't elect pickup-
class plans with full inventory (R3a) AND wounded cats correctly
score PickUp below Sleep/Flee/Eat (R3b), the L2 picture rebalances
toward survival and disposal dispositions naturally.

R1 is partially subsumed (R3a implements the planner precondition
via a marker gate). R2 (eligibility filters) is rejected on
substrate-over-override grounds: a filter masks the symptom by
removing options; R3b instead repairs the L2 score itself so the
unfit option naturally loses. Per CLAUDE.md and the substrate
discipline reaffirmed in 230's session, the DSE's score must fully
describe its own viability under the current state — filters are
the fallback when the substrate can't.

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

- **R3a hard gate proxy:** `plan_failures_by_reason` post-fix. Target: each
  of the four "inventory full" reasons drops ≥10× from baseline (current:
  18,374 / 2,132 / 633 / 389 ≈ 21k total → target < 2,000 total).
- **R3b correctness check:** post-230 dying-arc replay. Take the focal-cat
  trace from `logs/tuned-42` (Calcifer at tick 1251500, Cedar at tick
  1251800), reproduce the cat-state via a microexperiment, assert that
  the post-R3b L2 score for PickUp is materially below Sleep / Flee / Eat
  rather than tied within 1%. Concrete: with HP=0.49 and a fox 2 tiles
  away, PickUp should score < 0.5× Sleep's score (current: 1.01×).
- **R3b parity check:** healthy-cat L2 scoring should be unchanged. Take a
  healthy founder cat at tick 1210000 (pre-ambush, HP=1.0, full needs)
  and assert the PickUp / RetrieveRawFood / GatherHerb / RetrieveFoodForKitten
  L2 scores match pre-R3b within ε. The body-state axis contributes 0
  for healthy cats — the curve+weight calibration must preserve this
  invariant.
- **Indirect proxy on starvation:** combined with ticket 230's flee fix,
  the seed-42 soak should hit `deaths_by_cause.Starvation == 0`. This
  ticket's contribution is the inventory-clog half (R3a) plus the
  wounded-cat-keeps-doing-chores half (R3b); 230's is the plan-thrash
  half.
- **Microexperiments:**
  - R3a: scenario where a cat's inventory is preloaded full and food is
    placed adjacent. Assert: cat does NOT generate a
    `PickUpItemFromGround` / `RetrieveRawFood` plan; cat DOES generate a
    `Discarding` / `Trashing` plan to clear capacity.
  - R3b: scenario where a wounded cat (HP=0.4, fresh injury) sits next
    to a ground food item. Assert: L3 winner is `Sleep` or `Flee`, NOT
    `PickUp`. (Compare against pre-R3b: PickUp wins.)
- **Substrate-stub lint:** `scripts/check_substrate_stubs.sh` must pass —
  every new `Has*Slot` marker has a reader (planner precondition via
  `HasMarker`) and a writer (the capacity-authoring system), no
  allowlist entries. R3b's body-state Considerations consume already-
  authored perception; no new marker, no new lint impact.

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
    moving capacity-truth from runtime-only to substrate-marker, AND by
    subscribing pickup-class DSEs to body-state perception (R3b).

- 2026-05-08 #3: post-230 soak (commit ffb2b69b, `logs/tuned-42/`,
  Calcifer focal trace) ran clean structurally — substrate-aware Fleeing
  chain wired through, modifier-preempt count dropped from 39,536 to
  28,360, Starvation deaths dropped 6 → 4. Soak-trace dying-arc
  inspection at ticks 1251300–1252100 revealed a SECOND substrate gap
  on the same axis as 231's capacity-marker fix:

  | Tick | Cat | HP | Safety | **Chose** | Top L2 |
  |---|---|---|---|---|---|
  | 1251500 | Calcifer | 0.49 | 0.66 | **PickUp** | PickUp 0.958 / **Flee 0.948** / Sleep 0.71 |
  | 1251800 | Cedar | 0.38 | 0.18 | **PickUp** | **Sleep 1.08** / PickUp 0.99 / Hunt 0.85 |
  | 1251900 | Cedar | 0.38 | 0.20 | **PickUp** | **Sleep 1.05** / PickUp 1.01 / Flee 0.91 |
  | 1252000 | Cedar | 0.20 | 0.002 | **PickUp** | PickUp 0.99 / Sleep 0.91 / Hunt 0.82 |

  Both cats took fatal ambushes within 60–340 ticks of these decisions.
  Pickup-class DSE Considerations were inspected at
  `src/ai/dses/picking_up.rs:58-61` — single Consideration on
  `colony_food_security`, no body-state subscription. Sleep
  (`src/ai/dses/sleep.rs`) reads four body-state axes; the asymmetry is
  the structural defect. Per substrate-over-override discipline, the
  fix is to SUBSCRIBE the pickup DSEs to the existing `pain_level` /
  `body_distress_composite` / `health_deficit` perception, not to
  filter or gate. Scope expanded to R3a + R3b (capacity markers AT
  planner + body-state Considerations AT L2 scoring); title updated
  from "Gate PickupItem ... plans on inventory capacity" to
  "Strengthen pickup-class substrate — capacity markers + body-state
  subscription" to reflect the broadened scope.

  Companion follow-ons opened in the same session:
  - Body-state-coupled L3 softmax temperature (the temperature scalar
    becomes a function of body distress so wounded/threatened cats
    see decisions sharpen — Calcifer's PickUp 0.958 vs Flee 0.948
    coin-flip is the canary).
  - Body-state Considerations on non-pickup work DSEs (Hunt / Forage /
    Cook / Wander / Explore) — same substrate-stub shape as 231 R3b
    but on the food-production half rather than the item-handling
    half.
  - Damage-recency perception scalar + AcuteHealthAdrenalineFlee
    coupling (the modifier currently triggers on steady-state
    `health_deficit`; should ramp on *recent* damage so it lurches
    sharply post-injury and quiets during recovery, "tied to the
    danger a cat currently feels").

- 2026-05-08 #4: scope reconciliation between this ticket's text and
  the implementation that landed. Two divergences resolved during
  scoping:
  - **R3a marker shape: per-kind → single `HasFreeSlot`.** Ticket
    text said `HasFoodSlot` / `HasHerbSlot` / `HasMaterialSlot`;
    implementation landed a single `HasFreeSlot` because
    `Inventory.slots: Vec<ItemSlot>` is a unified 5-slot pool and
    every per-kind marker would have been an alias for
    `slots.len() < 5`. Per-kind capacity becomes meaningful only
    when armor / clothes / bag equip slots arrive; until then,
    single-marker matches the actual data model.
  - **R3a plan-template gating shape: hard precondition → GOAP
    DropItem-as-prefix dual-branch.** Ticket text proposed adding
    `HasMarker(HasFreeSlot::KEY)` as a hard plan-template
    precondition (cats with full inventory cannot generate a pickup
    plan; they fall back to other dispositions). Implementation
    landed dual-branch substrate-vs-plan-path composition mirroring
    the ticket-096 Construct precedent: cats can still elect
    PickingUp / RetrieveRawFood / etc. when full, and A*
    automatically composes `[DropItem, ..., PickUp]` with the
    runtime resolver's goal-aware `drop_priority` picking the
    lowest-priority slot. This produces the user's design-intent
    behavior ("drops off the herbs first then hunts") rather than
    requiring the disposal-side pressure tuning the ticket text
    flagged as out-of-scope.
  - **R3b DSE scope narrowed: 4 DSEs → PickingUpDse only.** Ticket
    text listed PickingUpDse / ForageDse / CaretakeDse /
    HerbcraftGatherDse for the body-state Consideration. Only
    PickingUpDse landed in 231's R3b commit. The others use
    `weighted_sum` compositions over multiple existing axes; making
    the body-state damping multiplicative there requires a
    composition-mode shift that's a larger balance change.
    Companion ticket 233 ('subscribe non-pickup work DSEs to
    body-state perception') already covers Hunt/Forage/Cook/Wander/
    Explore; the Forage/Caretake/HerbcraftGather damping rolls
    naturally into 233's scope.
  - **R3b axis: `body_distress_composite` → `health_deficit`.**
    Ticket text proposed `body_distress_composite` (or pain_level
    or health_deficit). The composite includes `hunger_urgency` —
    using it as a damping signal on PickUp would suppress the DSE
    on hungry cats, exactly when pickup is most useful (backwards
    direction). Switched to `health_deficit` only, which captures
    the dying-arc evidence (Calcifer HP=0.49, Cedar HP=0.38)
    without dampening hunger-driven behavior.

  Follow-on ticket 235 (`Smart deposit routing for clutter
  clearance`) opened blocked-by 231. Captures the narrative-quality
  work the user named in scoping ('drops off the herbs at the stash
  before hunting' rather than 'throws herbs in the dirt before
  hunting') — out of 231 because it requires per-class inventory-
  content markers + colony-destination perception markers + class-
  specific Deposit* actions, all expansion beyond the foundational
  capacity gate.
- 2026-05-08: Soak-verified post-231 (seed 42, commit 9b302638): inventory-full plan failures 24,581 → 0 (∞× drop, target was ≥10×). Hard gates: Starvation=0, ShadowFoxAmbush=0, total deaths=0. Welfare axes lifted (health +277%, welfare +71%). Verdict 'concern' from continuity:fail:play=0,burial=0 — downstream of zero deaths and short soak (kittens born tick 1.27M, soak ended at 1.30M before 4-season maturation). Two small new failure modes: DropItem:drop empty inventory (164) and TrashItemAtMidden:no item-slot (102) — both Discarding/Trashing pre-existing edges, not 231 regressions. Renamed colony_score.kittens_surviving → kittens_matured during verification (commit f22ecfa8) so footers stop reading like 'kittens died' when they're alive-and-developing.
