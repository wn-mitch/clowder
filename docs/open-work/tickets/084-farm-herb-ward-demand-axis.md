---
id: 084
title: Herb-stash economy + stash-low signal driving Farm DSE and coordinator build-pressure
status: in-progress
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-04-29
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [027-l2-pairing-activity.md, 084-farm-herb-ward-demand.md, 085-gardens-multiuse-build-gate.md]
landed-at: null
landed-on: null
---

## Why

The original 084 axis (Farm DSE reads `farm_herb_pressure` scalar) is **structurally correct but empirically inert** on every soak since 2026-04-29. Two facts surfaced via layer-walk on 2026-05-19 reframe the problem:

1. **Thornbriar is ad-hoc fetch, not stash-and-buffer.** GOAP `HerbcraftSetWard` plan is `TravelTo(HerbPatch) → GatherHerb → SetWard`; `HasWardHerbs` is a per-tick live read of `inventory.has_ward_herb()` (`src/systems/items.rs:38–103`); cats hold thornbriar for at most one weaving cycle; `Stores` doesn't track herbs at all (`src/systems/items.rs:260+` only counts food deposits). Steady-state stockpile ≈ 0, so "stockpile low" carries no information. The original 084 scalar (`ward_strength_low && !thornbriar_available`) sees this transient state and fires at most for one tick per gather→weave cycle.

2. **The coordinator has no strategic-level perception of wild-harvest sustainability.** It sees only `wild_thornbriar_available` (boolean) and `any_cat_carrying_thornbriar` (boolean) at `src/systems/coordination.rs:930-933`. No time-series, no flow-rate, no chronicity. The 085 v2-loose probe showed any aggressive loosening of this gate breaks survival canaries (`courtship 764→0`, `wards_placed 5→1`, 4 wildlife-combat deaths) by over-building. The gate is brittle by design and inadequate as a strategic signal.

Post-382 verification on `logs/tuned-42/` (commit 3e0153fe, 2026-05-18) confirms the 382 placement landing did not unblock 084: FarmDse evaluated 7,666 times across the run, score = 0.0 every tick, gated at L2 by `HasGarden::passed = false` for the entire run (no Garden ever constructed). 382 fixed the *placement* layer but the bottleneck is upstream at the pressure-accumulation layer.

**Architectural reframe (this iteration):** refactor the herb economy into stash-and-retrieve, mirroring the existing food→Stores→retrieve loop and the Caretake retrieve pattern (`src/steps/disposition/retrieve_any_food_from_stores.rs`). `Forage(gather)` deposits thornbriar into Stores; `SetWard` retrieves from Stores as its first step. Once stash levels are observable, the original 084 framing ("Farm DSE reads ward-stockpile demand") becomes meaningful: a colony-scoped marker `ColonyThornbriarChronicallyLow` fires when stashed thornbriar is below threshold over a chronicity window, driving both the coordinator's farming build-pressure gate and Farm DSE's herb-pressure axis. Mirrors 179's `ColonyStoresChronicallyFull` pattern.

## Scope

Staged across four commits, each gated on `just check && just test` clean before the next.

**Commit 1 — Stash infrastructure.**
- `StoredHerbs` component on every `Stores` building (`HashMap<HerbKind, u32>`); spawned at construction; capacity-bounded via new `stores_herb_capacity_per_kind: u32` constant (default 20).
- `resolve_deposit_herbs_to_stores` step resolver (mirrors `src/steps/disposition/deposit_at_stores.rs:58–177`).
- `resolve_retrieve_herbs_from_stores(kind)` step resolver (mirrors `src/steps/disposition/retrieve_any_food_from_stores.rs:41–85`).
- `GoapActionKind::DepositHerbs` + `GoapActionKind::RetrieveHerbs(HerbKind)` planner actions (mirror `DepositFood` / `RetrieveFoodForKitten`).
- `Feature::HerbsDeposited` (Positive) + `Feature::HerbsRetrieved` (Positive); classify + register.
- `HasStoredThornbriar` colony marker (writer in `update_colony_building_markers`).
- Unit tests: `stored_herbs_round_trip`, `stored_herbs_respects_capacity`, deposit/retrieve witnessed-on-success + unwitnessed-on-failure.

**Commit 2 — Plan templates: gather→deposit and retrieve→weave.**
- `HerbcraftGather` emit-plan extended: goal becomes `herbs_stashed` requiring `CarryingIs(Nothing) ∧ StoredHerbs(kind) > previous`. Planner sequences `[TravelTo(HerbPatch) → GatherHerb → TravelTo(Stores) → DepositHerbs]`.
- `HerbcraftSetWard` eligibility swap: `CanWard::KEY` → new `CanWardFromSupply::KEY` (combined marker firing when cat has thornbriar in inventory OR colony stash has thornbriar). Writer in `update_inventory_markers` / `update_colony_building_markers`.
- Plan branches naturally via GOAP precondition composition: `[TravelTo(WardSite) → SetWard]` when carrying, `[TravelTo(Stores) → RetrieveHerbs(Thornbriar) → TravelTo(WardSite) → SetWard]` when stash-only.
- `set_ward.rs:73` Fail path stays as defense-in-depth; rustdoc updated to note new plan-level precondition.
- Unit tests: gather-plan-ends-with-deposit, ward-plan-picks-carry-vs-retrieve, ward-DSE-ineligible-when-neither.

**Commit 3 — Stash signal + coordinator + Farm DSE wiring.**
- `ColonyThornbriarChronicallyLow` colony marker.
- `ThornbriarPressureTracker` resource (mirror `StoresPressureTracker`).
- `thornbriar_stash_low_threshold: u32` constant (default 3); reuses existing `chronicity_window_ticks`.
- Extend `update_colony_building_markers` to compute `total_stashed = Σ stored_herbs.count(Thornbriar)`; at window boundary, latch `chronic_low = total_stashed < threshold` and insert/remove the marker on the colony singleton.
- Coordinator gate at `coordination.rs:1073-1075`: replace `herb_demand = ward_strength_low && !wild_thornbriar_available && !any_cat_carrying_thornbriar` with `markers.has(ColonyThornbriarChronicallyLow::KEY, colony_entity)`. Drop now-unused `wild_thornbriar_available` (line 930) and `any_cat_carrying_thornbriar` (line 931).
- `FarmDse::FARM_HERB_PRESSURE_INPUT` scalar swap to `MarkerConsideration(ColonyThornbriarChronicallyLow::KEY, scoring.farm_herb_pressure_weight)`. Mirrors `BuildDse`'s chronic-full axis at `src/ai/dses/build.rs:103-107`.
- Drop now-unused `farm_herb_pressure` scalar entry at `src/ai/scoring.rs:895-907`.
- `MarkerSnapshot.set_colony` plumbing in `src/systems/goap.rs:1362-1408` (mirror `ColonyStoresChronicallyFull` block at lines 1405–1408).

**Commit 4 — Verification + balance doc + canary re-promotion.**
- `just soak-trace 42 Simba` → `logs/tuned-42-084/`; `just verdict logs/tuned-42-084` must pass hard gates.
- Inspect: `HerbsDeposited ≥ 1`, `HerbsRetrieved ≥ 1`, `WardPlaced ≥ baseline`, `CropTended` / `CropHarvested` ≥ 1 if regime entered, any Garden `BuildingConstructed`. `frame-diff` Simba per-DSE drift.
- Append iteration block to `docs/balance/084-farm-herb-ward-demand.md` (hypothesis / prediction / observation / concordance).
- If `CropTended` / `CropHarvested` fire naturally: re-promote in `src/resources/system_activation.rs:1179-1180` (flip `=> false` → `=> true`); update classification test. If not: keep demoted, document, open follow-on (multi-seed / forced-weather per 086's deferred paths).

## Out of scope

- **New `HerbStash` building kind.** Stash medium is extended `Stores` per design choice. Avoids cold-start (colonies already have founder Stores).
- **Promoting thornbriar to a real `Item` entity.** Keep lightweight `HerbKind` count (`HashMap<HerbKind, u32>`) — matches existing herb representation in `Inventory.slots`. "Items are real" pillar is honored where it currently is (food); revisiting herb representation is a separate substrate question.
- **Adding `colony_thornbriar_stash_low` axis to `HerbcraftGather` DSE** for proactive replenishment. Defer to follow-on unless commit-4 soak shows under-gathering.
- **`Feature::ThornbriarGatherCompleted` thornbriar-specific event** (vs the existing generic `Feature::GatherHerbCompleted`). Defer unless first soak shows non-thornbriar herb noise dominates the signal.
- **Calibration sweeps over `thornbriar_stash_low_threshold` and `farm_herb_pressure_weight`.** Default plausibility values for commit-3; `just hypothesize` follow-on if first soak surfaces over- or under-firing.
- **Restructuring the garden dual-purpose split** (FoodCrops vs Thornbriar). Both kinds remain.
- **Changing `CompositionMode::CompensatedProduct` on Farm.** The gate is correct shape.
- **Modifying the FoodCrops → Thornbriar repurposing gate at `coordination.rs:~530-540`.** Its hair-trigger predicate is intentional per 085's "asymmetric build-vs-repurpose gates" lesson.

## Approach

See per-commit detail under Scope. Bugfix-discipline layer-walk has been completed in this planning session and lives in `docs/balance/084-farm-herb-ward-demand.md ## Observation` plus the Why section above.

**Structural-option menu** (per CLAUDE.md Bugfix discipline; candidate that ships is **extend**):
- **split** — split `WardStrengthLow` into acute / chronic. Doesn't help; acute never fires in seed-42.
- **extend** *(chosen)* — extend the herb economy with stash-and-retrieve infrastructure; introduce `ColonyThornbriarChronicallyLow` chronic marker driven by stash level; rewire coordinator gate + Farm DSE axis to read it.
- **rebind** — rebind `herb_demand` to read `WardCoverageInsufficient` (influence-map ward gap signal). Doesn't address the per-cat herb-acquisition transient; orthogonal.
- **retire** — close 084 against 086's scenario integration test, leave the axis dormant. Punts the design problem; the user explicitly wants natural firing via strategic-coordinator perception.

**Key existing utilities reused:**
- `StoredItems` (`src/components/building.rs:342–421`) — capacity discipline pattern
- `StoresPressureTracker` (`src/resources/stores_pressure.rs`) — direct mirror for `ThornbriarPressureTracker`
- `update_colony_building_markers` (`src/systems/buildings.rs:478`) — extend in place
- `deposit_at_stores.rs:58–177`, `retrieve_any_food_from_stores.rs:41–85` — step-resolver templates
- `DepositFood` (`actions.rs:129-141`), `RetrieveFoodForKitten` (`actions.rs:858-924`) — planner action templates
- `MarkerConsideration::new` (`src/ai/considerations.rs:404-435`) — same wiring as Build's chronic-full axis
- `should_accumulate_farming_pressure` truth-table test (`coordination.rs:3963`) — keep, still valid against bool inputs

## Verification

Each commit gates the next. Per-commit details under Scope.

Soak acceptance:
- `just soak-trace 42 Simba && just verdict logs/tuned-42-084` — hard gates hold (Starvation = 0, ShadowFoxAmbush ≤ 10, four continuity canaries).
- `Feature::HerbsDeposited ≥ 1` and `Feature::HerbsRetrieved ≥ 1` (loop is exercised).
- `Feature::WardPlaced` rate ≥ baseline (no regression on the ward economy from the longer plan templates).
- `Feature::CropTended ≥ 1` and `Feature::CropHarvested ≥ 1` — only if stash-low regime is naturally entered.
- `Farming` PlanCreated count > 0 if regime entered.
- Continuity canaries hold (courtship / play / mentoring / grooming each ≥ 1; mythic-texture is pre-existing).
- `frame-diff` Simba vs prior baseline shows Farm DSE row movement consistent with the new wiring.

Balance doc appended per four-artifact methodology. Multi-seed sweep via `just hypothesize` only after single-seed passes naturally.

Canary re-promotion in `system_activation.rs::expected_to_fire_per_soak` (`Feature::CropTended` + `Feature::CropHarvested` flip to `=> true`) only after a passing soak that exercises the chain.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **309** (ready, items-crafting, score 0.89) — Herbcraft DSE reserve-deficit consideration — anticipatory ward / remedy crafti…
- ✓ landed **  6** (done, ai-substrate, score 0.86 (cross-cluster)) — Cluster-B shared spatial slow-state closeout
- ✓ landed **178** (done, ai-substrate, score 0.85 (cross-cluster)) — Balance-tune disposal DSEs from default-zero (176 follow-on)

<!-- linkages:end -->
## Log

- 2026-04-29: Opened. Carved out from ticket 083 (`l2-pairing-farming-scheduler-regression`) — 083 closes the L2 activation question by demoting the Farm canaries; 084 owns the herb-driven Farm-motivation thread that justifies eventual canary re-promotion.
- 2026-04-29: Code change landed (axis added to `FarmDse`, `farm_herb_pressure` scalar plumbed through `ctx_scalars`, commit `410f544c`). Signal choice: the same boolean condition the coordinator uses at `coordination.rs:532` (`ward_strength_low && !thornbriar_available`). Curve: Linear identity, weight 1.0, under existing `CompensatedProduct`.
- 2026-04-29: Treatment soak `logs/tuned-42-084/` ran against `410f544c`-dirty. **Outcome: bit-identical to baseline `tuned-42-083` on every farming-related metric.** P1–P4 are untestable in this seed; P5a–d (regression checks) all hold — the axis addition is empirically inert.
- 2026-04-29: **Parked.** Diagnosis (in `docs/balance/084-farm-herb-ward-demand.md ## Observation`): the colony never builds a garden in this regime, so `HasGarden` eligibility filter blocks Farm DSE before the new axis can affect anything. Root cause is upstream at `coordination.rs:984` — `pressure.farming` only accumulates when `food_fraction < 0.3`, but post-Wave-2 + L2-active colonies maintain `food_fraction = 0.98` from simulation start. The 084 axis is structurally correct and stays committed; canary re-promotion of `Feature::CropTended` / `Feature::CropHarvested` is **blocked-by: [085]** which owns the threshold-mismatch fix.
- 2026-04-30: 085 landed the disjunctive build-pressure gate (`food_demand || herb_demand`) as architectural prep — bit-identical to baseline in seed 42 (gate never fires in this seed's natural dynamics). Empirical investigation showed the gate works structurally (verified via probe: `food_threshold=0.95` + loosened `herb_demand=ward_strength_low` produces a Garden + `HasGarden = true`), but every loosening that makes the gate fire in seed 42 also breaks survival canaries (`courtship 764→0`, `wards_placed 5→1`, 4 wildlife-combat deaths) by redistributing cat-time from social/defense to construction. 084's `farm_herb_pressure` axis remains untestable on seed 42 because the colony never enters the `(ward_strength_low ∧ !ThornbriarAvailable)` regime long enough for Farm to score above competing actions — wild thornbriar respawns; cats gather on the rare absence. Re-blocked on **086** (find a triggering seed/scenario for the Farm canary, e.g. forced-weather destabilization or multi-seed sweep). 085's balance doc captures the empirical evidence at `docs/balance/085-gardens-multiuse-build-gate.md ## Why P1–P3 are unchanged in seed 42`.
- 2026-05-19: accuracy audit pass — parked status correct; FarmDse exists in src/ai/dses/farm.rs; code changes at 410f544c documented accurately.
- 2026-05-19: **Reframed and unparked.** Layer-walk against post-382 soak (`logs/tuned-42/`, commit 3e0153fe) confirmed 382's placement fix didn't unblock 084 — FarmDse scored 0.0 every tick across 7,666 evaluations, gated at L2 by `HasGarden::passed=false`. Root cause re-stated: thornbriar is ad-hoc fetch (no stash, no buffer), so the 085 supply-strict check measures a transient state that carries no strategic information. User-driven reframe: refactor herb economy into stash-and-retrieve mirroring the food→Stores→retrieve loop, then drive a `ColonyThornbriarChronicallyLow` marker off stash level (mirror 179's chronic-marker pattern). Title + scope expanded to cover stash mechanic + signal wiring. Design choices: stash extends existing Stores (not new building); thornbriar stays a lightweight `HerbKind` count (not promoted to Item entity); ticket covers both layers (not split). Status: parked → in-progress. Plan archived at `~/.claude/plans/let-s-pick-up-084-polymorphic-backus.md`.
