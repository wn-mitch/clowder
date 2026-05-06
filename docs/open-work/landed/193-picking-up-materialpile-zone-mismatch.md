---
id: 193
title: PickingUp plan template routes to MaterialPile but eligibility latches on Carcass — 3302 unreachable plans / soak
status: done
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 1e84c87d
landed-on: 2026-05-06
---

## Why

Post-wave seed-42 15-min canonical soak (`logs/tuned-42`, commit
parent `1a394467` working-copy with 188 changes) shows colony
collapse at 1 season survived (vs pre-wave 2 seasons), nourishment
0.22 (vs pre-wave 0.52, **-57%**), DeathInjury rate +147%/10kt.

The mechanism, identified by hunt-pipeline ratio walk pre vs
post-wave (rates per 10k ticks; PRE = `logs/tuned-42-post-178-original`,
POST = `logs/tuned-42`):

| Stage | Feature | PRE rate | POST rate | Δ |
|---|---|---:|---:|---:|
| Plan creation | **PickingUp:GoalUnreachable** | **0** | **1367.6** | **NEW** |
| Plan creation | Hunting:GoalUnreachable | 47.6 | 82.4 | +73% |
| Plan creation | Foraging:GoalUnreachable | 33.8 | 59.6 | +77% |
| Kill chain | CarcassSpawned | 38.9 | 23.2 | -40% |
| Storage | OverflowToGround | 467.9 | 956.7 | **+104%** |
| Disposal | ItemRetrieved | 6.10 | 2.07 | **-66%** |
| Cook | FoodCooked | 6.10 | 2.07 | **-66%** |
| Eat | FoodEaten | 14.54 | 14.50 | flat |

PickingUp wins L3 votes 1367×/10kt that A* cannot satisfy. Each
failure burns a cat-tick. Cumulative effect: cats churn through
replans, hunt-stalk completion drops (CarcassSpawned -40%),
overflow doubles (carcasses pile up because no one can pick them
up), cooked food drops -66%, nourishment crashes.

**The defect:** PickingUp's plan template routes through
`PlannerZone::MaterialPile`, which resolves only to ground items
where `kind.material().is_some()` (Wood / Stone build materials).
Carcasses are food, not material. The eligibility marker
`HasGroundCarcass` (185) latches on Carcass entities, but the
plan template can never resolve them.

The 176 author flagged this stub explicitly:

```rust
/// 176: plan template for `PickingUp` — single-step retrieval from
/// the ground. Reuses `MaterialPile` zone (the existing OnGround-
/// item zone resolution) until a more general `TargetGroundItem`
/// zone lands. Default-zero scoring keeps this dormant.
pub fn picking_up_actions() -> Vec<GoapActionDef> {
```

185 lifted the curve from default-zero **without landing the
`TargetGroundItem` zone**. Wave-closeout step 2 introduced the
defect; ticket 189's third reframe identifies it as the root
cause of seed-42 collapse, separate from the original
schedule-edge perturbation hypothesis.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/systems/buildings.rs::update_colony_building_markers` | `HasGroundCarcass` latches on any uncleansed/unharvested Carcass entity (185). Author is correct; reader (eligibility filter) is correct. | `[verified-correct]` |
| L1 query | `colony_state_query` in goap.rs / disposition.rs | Threads `Has<HasGroundCarcass>` into MarkerSnapshot. | `[verified-correct]` |
| L2 DSE scores | `src/ai/dses/picking_up.rs` | Inverted-Logistic on `colony_food_security`. Scores high when food security low + HasGroundCarcass set. Curve correct; the DSE wins L3 votes when conditions latch. | `[verified-correct]` |
| L2 eligibility | `picking_up.rs::eligibility` | `forbid(Incapacitated) ∧ require(HasGroundCarcass)`. Resolves the marker correctly; admits the cat to L2 scoring. | `[verified-correct]` |
| L3 softmax | `src/ai/scoring.rs:1704` | `score > 0.0` gate — PickingUp's nonzero score enters the L3 pool with positive softmax mass. | `[verified-correct]` |
| Action→Disposition mapping | `from_action(PickUpItemFromGround)` | Maps to `DispositionKind::PickingUp`. | `[verified-correct]` |
| **Plan template** | **`src/ai/planner/actions.rs:278-285` `picking_up_actions()`** | **Routes through `PlannerZone::MaterialPile`. The 176 stub comment names this as conditional ("until `TargetGroundItem` lands"). 185 left it unchanged.** | **`[verified-defect]`** |
| **Zone resolver** | **`src/systems/goap.rs:2275-2284`** | **`material_pile_positions` filters items by `kind.material().is_some()` — Wood/Stone only, NOT food/carcass. So `MaterialPile` resolves to zero matches when only carcasses are on the ground.** | **`[verified-defect]`** |
| Completion proxy | `src/components/commitment.rs` | PickingUp completion checks inventory delta on PickUpItemFromGround. No issue at this layer. | `[verified-correct]` |
| Resolver | `src/steps/disposition/resolve_pick_up_from_ground` | Takes a `target_entity`, picks it up if reachable. Works correctly when given a target. The dispatch arm at `goap.rs::PickUpItemFromGround` reads `plan.step_state[step_idx].target_entity` — the planner's failed TravelTo means target stays `None`. | `[verified-correct]` |

The defect lives at the **plan template ↔ zone resolver** boundary.
Eligibility, scoring, and resolver are all correct. The plan template
is honest about being a stub but 185 lifted the upstream score without
landing the downstream zone.

## Fix candidates

**Parameter-level options:**
- **R1 — drop the 185 curve lift** (revert PickingUp scoring to
  `Linear { slope: 0.0, intercept: 0.0 }`). The eligibility filter
  still gates on HasGroundCarcass, but the score is always zero so
  PickingUp never wins L3. Closes the regression but loses the wave's
  emergent-scavenging intent. Quickest revert.
- **R2 — add an inline target-resolution fallback in `PickUpItemFromGround` dispatch**
  (`src/systems/goap.rs:4815-4824`), mirroring `TrashItemAtMidden`'s
  pattern: when `plan.step_state[step_idx].target_entity.is_none()`,
  resolve the nearest carcass from `snaps.uncleansed_carcasses` (or a
  similar per-tick snapshot). The plan template stays as
  `MaterialPile`, but the dispatch arm fills in the carcass entity
  before invoking `resolve_pick_up_from_ground`. **Caveat:** the
  TravelTo step preceding PickUp still routes to MaterialPile zone, so
  the cat may travel to wrong (or no) tile. This option works only if
  the dispatch arm ALSO overrides the TravelTo target — which would
  require deeper plumbing.

**Structural options** (CLAUDE.md bugfix-discipline requires ≥1):

- **R3 (split) — add `PlannerZone::CarcassPile`.** New zone in
  `src/ai/planner/mod.rs` PlannerZone enum. Resolver in
  `goap.rs::resolve_zone` (around line 6394) returns
  `(entity, position)` from `snaps.uncleansed_carcasses` (already a
  per-tick collection in coordination.rs). `picking_up_actions()`
  routes through `CarcassPile` instead of `MaterialPile`.
  Cleanest; matches the 176 stub's named successor (`TargetGroundItem`
  generalized to a Carcass-specific zone). Mirrors the existing
  `MaterialPile` / `ConstructionSite` / `Stores` / `Hearth` / `Garden`
  / `Kitchen` zone pattern.
- **R4 (extend) — generalize `MaterialPile` filter.** Drop the
  `kind.material().is_some()` filter in goap.rs:2275-2284 and let
  any OnGround item count as a MaterialPile. Bad — semantics conflate
  "haulable build material" with "any ground item," and other
  consumers of MaterialPile (Build chain) would suddenly treat
  carcasses as building materials.
- **R5 (rebind) — route picking_up_actions through `PlannerZone::Stores`.**
  Bad — Stores resolves to building positions, not item positions.
- **R6 (retire) — drop the PickingUp DSE entirely.** Bad — 185 added
  it for a reason (load-bearing for the kill→carcass-on-ground→pick-up
  flow once `engage_prey` overflow lands real entities). Retiring
  reverts the wave's emergent-scavenging intent.

## Recommended direction

**R3 (split — add `PlannerZone::CarcassPile`)** is the structural
answer. The 176 stub explicitly named this as the deferred
follow-on. The wave didn't ship it; this ticket does.

R1 (revert curve) is a defensible interim if R3 is too risky to
land in one commit; R3 supersedes it. R2 (inline fallback) was
considered but the TravelTo step's target dependency makes it
fragile — a structural zone is cleaner.

R3 implementation sketch:
- `src/ai/planner/mod.rs` — add `PlannerZone::CarcassPile`.
- `src/systems/goap.rs::resolve_zone` (around 6394) — handle
  `CarcassPile`: pick the nearest entry from `snaps.uncleansed_carcasses`
  or compose from the existing `carcasses` query.
- `src/ai/planner/actions.rs::picking_up_actions` — replace
  `PlannerZone::MaterialPile` with `PlannerZone::CarcassPile`.
- Update the 176 stub comment to remove the "until TargetGroundItem
  lands" disclaimer.
- New scenario: `picking_up_scavenging` (deferred from 185 to 191;
  this ticket should land it inline since the scenario directly
  exercises the fix).

## Out of scope

- Generalizing CarcassPile to a broader `TargetGroundItem` zone that
  also covers herbs / dropped equipment — separate ticket if/when
  those use cases land.
- Tuning PickingUp's curve (deferred to 191).
- The L2 handing_target_dse follow-on (deferred to 192).

## Verification

- Re-run `just soak 42` post-fix; verify:
  - `PickingUp:GoalUnreachable` count drops to ~0 (or near-zero —
    only when no carcass exists despite the marker, which should be
    rare).
  - `ItemRetrieved` rises substantially (target: ≥10 / 10kt, vs the
    current 2.07 / 10kt regression).
  - `OverflowToGround` rate drops back toward pre-wave (~470 / 10kt
    or below).
  - `seasons_survived ≥ 2` (matches pre-wave baseline).
  - Survival hard-gates pass.
  - Continuity canaries: courtship / mythic-texture rise above 0
    (currently dark in the post-wave soak).
- New scenario test: `picking_up_scavenging` — one cat, empty
  inventory, three carcasses on ground, low food security. Cat must
  elect PickingUp and complete TravelTo + PickUp without GoalUnreachable.
- `just frame-diff` between pre-fix and post-fix focal-cat traces:
  PickingUp's plan-failure rate should drop to noise.

## Log

- 2026-05-06: opened. 189's third reframe (post-soak diagnosis)
  identifies this as the actual root cause of seed-42's post-wave
  collapse. The first two 189 reframes (RNG noise → scoring-substrate
  expansion) were directionally correct but missed the concrete
  defect: 185 lifted PickingUp's curve without landing the
  `TargetGroundItem` zone the 176 stub named as a prerequisite.
  Hunt-pipeline ratio walk in `~/.claude/plans/i-just-finished-a-compiled-hanrahan.md`-related
  diagnostic shows 1367/10kt PickingUp:GoalUnreachable failures
  introduced by the wave (NEW failure mode), driving the cascade.
- 2026-05-06: landed (R3 — added `PlannerZone::CarcassPile`). The
  user's "items are real" framing pinned the architectural
  direction: the new zone resolves to OnGround food `Item` entities
  (engage_prey overflow today; carcass-as-container child Items
  tomorrow) rather than `Carcass` component entities. Concretely:
  added `PlannerZone::CarcassPile`; threaded `food_pile_positions`
  through `build_zone_distances` / `resolve_zone_position` /
  `classify_zone` / `build_planner_state` / `StepSnapshots` /
  `resolve_travel_to`; rerouted `picking_up_actions` from
  `MaterialPile`; filled `target_entity` from snapshot in the
  `PickUpItemFromGround` dispatch arm (TrashItemAtMidden pattern);
  re-wired `HasGroundCarcass` authoring in
  `update_colony_building_markers` to gate on the OnGround food-Item
  surface (the resolver's actual contract). Surfaced & fixed a
  latent inconsistency in `transfer_item_inventory_to_stored`
  (`src/components/item_transfer.rs`) — the function spawned the
  destination Item with `ItemLocation::OnGround` and never
  reconciled to `StoredIn(dest_entity)`, which became visible only
  once the marker scanned for OnGround food-Items as a substrate
  signal. New scenario `picking_up_scavenging` exercises the fix
  end-to-end (PickUp wins L3 by tick ~11, resolver despawns the
  Item, inventory rises). All canary tests + lints + 1906 unit
  tests pass.
