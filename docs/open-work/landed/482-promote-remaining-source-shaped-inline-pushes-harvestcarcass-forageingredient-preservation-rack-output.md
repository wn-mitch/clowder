---
id: 482
title: Promote remaining Source-shaped inline pushes — HarvestCarcass / ForageIngredient / Preservation rack output
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-27
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 325189cb1f2b
landed-on: 2026-05-28
---

## Why

429 landed the `ItemSource` trait + 4 impls covering the 7 inline `inventory.pouch.push(...)` sites named in its Log. But the lint-extension audit surfaced three more Source-shaped sites that 429 deferred to keep its scope bounded:

- `src/systems/preservation.rs:127` — drying-rack / smoking-rack completion spawns a fresh `Item` on the ground via `commands.spawn(Item::with_modifiers(...))`. This is a Source (the dried/smoked item enters the world from the rack's preserved state).
- `src/systems/goap.rs::resolve_harvest_carcass` (~L7482) — `HarvestCarcass` GoapAction calls `inventory.add_item_with_modifiers(ShadowBone, …)` to materialize a magic-substrate output. Source (item enters the actor's inventory from a carcass-harvesting ritual).
- `src/systems/goap.rs::resolve_forage_item` ingredient arm (~L10042) — herbcraft-ingredient foraging spawns the ingredient as an OnGround Item via `commands.spawn(Item::with_modifiers(...))` rather than pushing into inventory. Source-with-ground-placement (an unusual variant — the cat doesn't pick up the ingredient, they make it available for later pickup).

Each violates the items-are-real Sources gate in the same way the 7 sites 429 promoted did: inline mutation, no `ItemSource` trait dispatch, no per-Source `Feature::*` witness. They survive in `scripts/item_transfers.allowlist` with ticket-429-as-rationale until this ticket lands their trait impls.

## Scope

- `PreservationOutputSource` — `impl ItemSource` for the rack-output spawn. Witness: `Entity` (the spawned ground item). Feature: `Feature::ItemSourcedFromPreservation` (Positive, expected:true — preservation completions fire reliably in seed-42).
- `HarvestCarcassSource` — `impl ItemSource` for the ShadowBone yield. Witness: `ItemKind`. Feature: `Feature::ItemSourcedFromHarvestCarcass` (Positive). The existing `CarcassHarvested` Feature stays as the gameplay-event witness; the new Source Feature is the items-are-real gate witness (different concerns — same as `ByproductSpawned` vs `ItemSourcedFromHuntCatch`).
- `ForageIngredientSource` — `impl ItemSource` for the herbcraft-ingredient ground spawn. Witness: `Entity`. Feature: `Feature::ItemSourcedFromForageIngredient` (Positive). Special-case: this Source always places on ground, never inventory — the trait's default `source()` body needs an override or a hint flag so the inventory-push arm is structurally bypassed. Cleanest expression: an `always_ground: bool` knob on `ItemSource`, or a parallel `GroundOnlySource` trait. Pick whichever reads cleanest in the existing item_gate.rs module.
- Drop the three `scripts/item_transfers.allowlist` entries that 429 added for these sites — the trait dispatches replace the bypass.
- Enroll the three new Feature variants in `Feature::ALL`, `Feature::category`, `Feature::expected_to_fire_per_soak`, `Feature::feature_name`, and bump `EXPECTED_VARIANT_COUNT` + the per-category test count in `system_activation.rs::tests`.

## Out of scope

- Behavior-level tuning of preservation / harvest / ingredient yields — this is a substrate-codification ticket, not balance work.
- New Source kinds (trader arrivals are parked in 381; world-init founder spawns are auto-skipped by the linter under `src/world_gen/**` and stay there).
- `ItemTransfer` / `ItemSink` trait retrofit — separate follow-on (the existing function-shape resolvers under `src/steps/disposition/**` already satisfy the contract behaviorally; unifying them to a trait is a parallel work item).

## Current state

Opened 2026-05-27 at 429's landing. The three sites are tracked in `scripts/item_transfers.allowlist` with `429` as the ticket id — that should rotate to this ticket's id when work begins. Confirm via `bash scripts/check_item_transfers.sh` that the lint passes pre-work; the allowlist should let it.

## Approach

Mirror 429's Commit 2b structure: one file per Source under `src/components/item_gate/sources/`. Each impl declares `kind() / modifiers() / ground_quality() / ground_position()` (defaults as needed) and `const FEATURE`. Call sites become a 5–10-line trait dispatch with `record_if_witnessed` for the Feature emission and the secondary `OverflowToGround` emission when the placement witness reads `Ground`.

The `ForageIngredientSource` always-ground variant is the only design-time decision — pick the cleanest expression in `item_gate.rs`. Recommended: add a `placement_policy(&self) -> SourcePlacementPolicy` getter (enum `{InventoryFirst, AlwaysGround}`) and branch the default `source(...)` body on it. Keeps the inventory-push arm structurally absent for the ingredient case.

## Verification

- `just check` — strict items-are-real linter passes without the three allowlist entries.
- `just test` — scenarios under `src/scenarios/drying_chain_eligibility.rs` (which already exercise preservation completion) confirm the new `ItemSourcedFromPreservation` Feature fires; `harvest_carcass`-related scenarios (or a new one) confirm the ShadowBone Source fires; herbcraft-ingredient pickup scenarios confirm the ground-spawn Source fires.
- `just soak-trace 42 Simba` + `just verdict logs/tuned-42-<sha>` — the three new Source Features appear in `SystemActivation.counts` ≥ 1× each. No drift on survival/continuity canaries (substrate refactor, behavior-neutral).
- Optional: `just frame-diff` against a paired-archive at this ticket's parent — no per-DSE drift on the Eat/Cook/Herbcraft/Harvest DSEs.

## Log

- 2026-05-27: opened as a 429 follow-on. Three sites named in 429's landing log + allowlist; each is structurally a Source today but bypasses the named gate. Promotion mirrors 429's Commit 2b shape — minimum-viable trait impls + Feature enrollment + call-site swap + linter allowlist cleanup.
- 2026-05-28: 2026-05-28: landed in two commits. Commit 1 extends ItemSource with SourcePlacementPolicy (InventoryFirst / AlwaysGround), changes SourceCtx::inventory to Option<&mut Inventory> for fixture-emit sites, ships PreservationOutputSource / HarvestCarcassSource / ForageIngredientSource + three Feature variants. Commit 2 swaps the three call sites and drops the allowlist entries. HarvestCarcass retired a silent-drop (inventory.add_item_with_modifiers's false return was ignored on full pouch). HarvestCarcass classification: expected_to_fire_per_soak=false to mirror its 1:1 sibling CarcassHarvested; the other two enrolled true after soak verification (logs/tuned-42-d531318e, promoted as post-482-source-promotions baseline).
