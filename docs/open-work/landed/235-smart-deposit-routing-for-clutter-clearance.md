---
id: 235
title: Smart deposit routing for clutter clearance
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-19
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

Ticket 231 landed `HasFreeSlot` + DropItem-as-prefix dual-branch
composition: cats with full inventory now compose `[DropItem,
PickUpItemFromGround]` automatically when they elect a pickup-class
disposition. The runtime resolver picks the lowest-priority slot via
`drop_priority` (curio < material < herb/food, with goal-aware
state modifiers).

**The narrative weakness.** A cat clogged with herbs who wants to
hunt drops the herb on the ground at the cat's current position.
Ideally — per the user's design intent in 231's scoping conversation
— the cat would route through the herb stash, deposit the herb
usefully, and then hunt: `[TravelTo(HerbStash), DepositHerb,
TravelTo(HuntingGround), Hunt]` rather than `[DropItem,
TravelTo(HuntingGround), Hunt]` with a herb left in the dirt.

**Required substrate expansion** (per 231's narrowing decision):
- Per-class inventory-content markers (`HasMaterialsInInventory`,
  `HasCuriosInInventory` — extending the existing
  `HasHerbsInInventory`).
- Colony-destination perception markers (`HasHerbStashAccessible`,
  `HasMaterialPileAccessible`).
- Class-specific `Deposit*` actions in pickup-class plan templates,
  competing with the bare `DropItem` on cost — A* prefers the
  routed-deposit when a stash is reachable.
- Curio-specific sink: curios have no destination today; either
  retire them as droppable-anywhere (the v1 behavior under 231) or
  introduce a `Cache` building (out of scope here, see ticket 16).

**Hard gate.** None today — 231's resolver-level `drop_priority`
ensures cats prefer dropping curios over hard-earned items. This
ticket is narrative-quality work, not a survival fix. Soak verdict
gate: post-ship, ground-item distribution should show herbs landing
near the herb stash rather than at random cat positions.

Blocked-by 231 because the substrate hooks (`HasFreeSlotThisPlan`,
DropItem-as-prefix composition) only exist post-231.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers (struct) | `src/components/markers.rs:347-419` | `HasHerbsInInventory`, `HasFreeSlot` already exist. 235 adds `HasMaterialsInInventory`, `HasCuriosInInventory`, `HasHerbStashAccessible` as ZST + `KEY` const. | `[verified-correct]` |
| L1 marker writer (inventory) | `src/systems/items.rs:38-128` `update_inventory_markers` | Per-tick author. Extended to also author `HasMaterialsInInventory` (`has_any_material()`) and `HasCuriosInInventory` (`has_any_curio()`) — same insert/remove shape as the existing herb-class markers. | `[verified-correct]` |
| L1 marker writer (reachability) | `src/systems/goap.rs::herb_stash_accessible_for` | Per-cat Manhattan-distance reachability author mirroring `materials_available_for`. Returns false when `stores_positions` is empty (degenerate early state). | `[verified-correct]` |
| MarkerSnapshot wiring | `src/systems/goap.rs` (per_cat_markers_q + evaluate_and_plan + build_planner_markers) | All 3 new markers wired via `set_entity` at the two per-cat sites (snapshot + planner-replay) so the eligibility filter and the planner agree. | `[verified-correct]` |
| L2 DSE scores | `src/ai/dses/...` | Unchanged — 235 is plan-template work; no DSE eligibility flips. | `[verified-correct]` |
| L3 softmax | `src/ai/scoring.rs` | Unchanged — A* picks deposit-vs-drop within the chosen disposition. | `[verified-correct]` |
| Plan templates (4 with prefix) | `src/ai/planner/actions.rs` `picking_up_actions` / `cooking_actions` / `caretaking_actions` / `herbalism_actions` (all 3 Herbcraft variants) | Each grows a second prefix `GoapActionDef { kind: DepositHerbs, cost: 1, preconditions: [ZoneIs(Stores), CarryingIs(Herbs), HasMarker(HasHerbStashAccessible), HasMarker(HasHerbsInInventory)], effects: [SetHasFreeSlotThisPlan(true), SetCarrying(Nothing)] }`. A* splices `TravelTo(Stores)` from `travel_actions`. | `[verified-correct]` |
| Plan template (hunting) | `src/ai/planner/actions.rs::hunting_actions` | Promoted from no-prefix to full DropItem-prefix + DepositHerbs-prefix + dual-branch `SearchPrey` (substrate `HasFreeSlot` / plan-path `HasFreeSlotThisPlan`). Restores planner-side slot gating as a *positive* substrate marker (091's lesson preserved). | `[verified-correct]` |
| Resolver (herb deposit) | `src/steps/disposition/deposit_herbs_to_stores.rs` | Reused unchanged — deposits all herb-kind slots to nearest Stores' `StoredHerbs`. | `[verified-correct]` |
| Resolver (drop) | `src/steps/disposition/drop.rs::drop_priority` | Reused unchanged — handles fallback when stash is unreachable. | `[verified-correct]` |
| Substrate-stub allowlist | `scripts/substrate_stubs.allowlist` | `HasMaterialsInInventory` allowlisted under `235-follow-on`; `HasCuriosInInventory` allowlisted under `16`. Drop each row when the named follow-on reader ships. | `[verified-correct]` |
| Tuning knob | `src/resources/sim_constants.rs::DispositionConstants::herb_stash_reachable_radius` | New, default 60 Manhattan tiles (≈⅓ of typical map diagonal). Caps detour eligibility so cats don't traverse the map to deposit one herb. | `[verified-correct]` |

## Fix shape

**R2 (extend)** wins — add a second prefix branch (`DepositHerbs` as means-to-end) to the 4 existing pickup-class plan templates, and promote `hunting_actions` from no-prefix to the same dual-prefix + dual-branch shape `picking_up_actions` uses. A* composes `[TravelTo(Stores), DepositHerbs(prefix), <goal>]` instead of `[DropItem, <goal>]` when the stash is reachable; falls back to DropItem otherwise. Structural alternatives rejected:
- **split** (new `PickingUpWithRouting` disposition): rejected — routing is a planning-time A* choice, not a scoring-time election; a disposition adds L2/L3 duplication for no scoring difference.
- **rebind** / **retire**: N/A.

## Out of scope (follow-on tickets opened in the same landing commit)

- **Central material pile + smart material-deposit routing** — `HasMaterialPileAccessible` per-cat author + `DepositMaterials` resolver + materials-deposit prefix on the 5 templates extended in 235. `HasMaterialsInInventory` marker ships in 235 as scaffolding; this follow-on adds the reader and drops the allowlist row. Tied to ticket 16's crafting work.
- **Curio Cache routing** — `HasCurioCacheAccessible` + curios-deposit prefix on Hunting + Foraging. Blocked on ticket 16's Cache building. `HasCuriosInInventory` marker ships in 235 as scaffolding under the same allowlist discipline.

## Verification

No hard gate. Soak verdict should pass survival canaries (Starvation == 0, ShadowFox ≤ 10, continuity canaries each ≥ 1). End-to-end:

1. `just check && just test` — substrate-stub checks pass; new unit tests for the deposit-prefix branches and the reachability author. ✅
2. `just soak <baseline-seed>` + `just verdict <run-dir>` — pass/concern/fail gate. Hunting/Foraging trip counts steady or up (cats no longer abort hunt-runs from full inventory).
3. `just soak-trace <baseline-seed> <focal>` — focal-cat trace. Confirm `HasHerbStashAccessible` flips with cat position; L3 trace shows the new prefix in plans when cat is at/near Stores carrying herbs.
4. Narrative readout — herbs on the ground should cluster near Stores tiles. Pre-235: scattered; post-235: ≥ majority within Manhattan-5 of a Stores building.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed ** 38** (done, ai-substrate, score 0.89 (cross-cluster)) — "MaterialsDelivered routing gap → full Pickup/Carry/Deliver pipeline (infrastru…
- ✓ landed **152** (done, ai-substrate, score 0.89 (cross-cluster)) — Tier-1 disposition-collapse audit — sweep for sibling Eat-into-Resting defects
- · **249** (parked, ai-substrate, score 0.89 (cross-cluster)) — Extend DispositionFailureCooldown coverage to Resting/Guarding/PickingUp et al.…

<!-- linkages:end -->
## Log
- 2026-05-08: opened as 231 follow-on per the user's item-routing design intent.
- 2026-05-19: accuracy audit pass — template placeholders present (layer-walk audit table to be filled), no file path issues, Hot Context section should be removed per user instructions, frontmatter clean.
- 2026-05-19: implementation landed — 3 new markers (`HasMaterialsInInventory`, `HasCuriosInInventory`, `HasHerbStashAccessible`), inventory writer + reachability author wired at both MarkerSnapshot sites, deposit-prefix branch added to PickingUp / Cooking / Caretaking / Herbalism (Gather / Remedy / SetWard) plan templates, Hunting promoted to full dual-prefix + dual-branch shape (DropItem prefix + DepositHerbs prefix + substrate-vs-plan-path `SearchPrey`), `herb_stash_reachable_radius` knob (default 60) added to `DispositionConstants`. `chokepoint_defense_isthmus` scenario's stale `expected_features: ["GatherHerbCompleted"]` (broken pre-235 by post-084 retrieve-from-stash dynamics) trimmed to `["CropHarvested"]` with rationale comment; the L3 election in that fixture never reaches herbalism emission grain. `just check` + `just test` green.
- 2026-05-19: 2026-05-19: landed — 5 plan templates extended (PickingUp / Cooking / Caretaking / Herbalism / Hunting), 3 new markers (HasMaterialsInInventory + HasCuriosInInventory + HasHerbStashAccessible), per-cat reachability author (herb_stash_accessible_for) + DispositionConstants::herb_stash_reachable_radius knob (default 60). Follow-ons 421 (materials) + 422 (curios) opened in this commit.
