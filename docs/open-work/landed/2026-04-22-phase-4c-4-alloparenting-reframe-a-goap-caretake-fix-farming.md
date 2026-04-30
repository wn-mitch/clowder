---
id: 2026-04-22
title: Phase 4c.4 — Alloparenting Reframe A + GOAP Caretake fix + Farming canaries
status: done
cluster: null
landed-at: 1261224
landed-on: 2026-04-22
---

# Phase 4c.4 — Alloparenting Reframe A + GOAP Caretake fix + Farming canaries

Bundle of four structurally-related fixes, all discovered during
the Reframe A hypothesis test. Phase 4c.3 had claimed to wire
KittenFed but it had stayed `= 0` in every soak — this phase
unblocked it across three distinct failure layers and lit the
farming system that had been quietly dead since its introduction.

**(1) Bond-weighted compassion — alloparenting Reframe A (~150 LOC).**
- `CaretakeResolution` now surfaces the target kitten's mother /
  father (`src/ai/caretake_targeting.rs:72-82`). New
  `caretake_compassion_bond_scale` helper computes
  `1 + max(0, fondness_with_mother) × boost_max` given a
  closure-style fondness lookup. Non-parents with a strong bond to
  mama get amplified compassion; hostility clamps at baseline 1.0
  so "I hate mama" can't suppress compassion below colony norm.
- New `ScoringContext.caretake_compassion_bond_scale` field flows
  through `ctx_scalars` as a dedicated input key
  (`"caretake_compassion"`), distinct from the shared `"compassion"`
  axis that `herbcraft_prepare` reads. `CaretakeDse::COMPASSION_INPUT`
  points at the caretake-local key — bond-weighting amplifies only
  care-for-hungry-kitten decisions.
- Populate sites wired at both `disposition.rs:evaluate_dispositions`
  (+`:disposition_to_chain` for chain building) and
  `goap.rs:evaluate_and_plan`. Reads `Relationships::get(adult,
  mother).fondness`. New `SimConstants.caretake_bond_compassion_boost_max`
  (default 1.0 → doubled compassion at max fondness).
- 7 new unit tests on `caretake_compassion_bond_scale` covering
  no-target / self-as-mother / fondness amplification / hostile
  clamp / missing relationship / father-fallback.

**(2) GOAP Caretake plan was silently half-shipped (Phase 4c.3
remnant).** Phase 4c.3's landing entry claimed to "rewrite
`build_caretaking_chain` for physical causality" into a 4-step
retrieve→deliver chain — true, but only in the unscheduled
disposition-chain path. The scheduled GOAP path's
`caretaking_actions()` still emitted `[TravelTo(Stores),
FeedKitten]` with no retrieval step, and `resolve_feed_kitten`
advanced silently when `inventory.take_food()` returned `None`.
This is why `KittenFed = 0` even across the pre-fix v2 soaks.
- New `GoapActionKind::RetrieveFoodForKitten` + step handler in
  `systems/goap.rs`; new `resolve_retrieve_any_food_from_stores`
  step helper (mirrors the raw-only sibling but accepts cooked
  food too — kittens eat either form).
- `caretaking_actions()` now emits two actions: retrieve (precond
  `ZoneIs(Stores)`; no `CarryingIs(Nothing)` — that gate blocked
  plans when adults had herbs or foraged food on arrival) → feed
  (precond `ZoneIs(Stores) + CarryingIs(RawFood)`; effect
  `SetCarrying(Nothing) + IncrementTrips`). Planner unit tests
  assert the three-step `[TravelTo(Stores),
  RetrieveFoodForKitten, FeedKitten]` shape including the
  "carrying herbs at start" regression case.

**(3) Target-kitten entity persistence at plan creation.** Even
with the retrieve step wired, the first soak still showed
`KittenFed = 0` on 66 Caretake plans with the correct chain
shape. Root cause: the FeedKitten handler re-ran `resolve_caretake`
from the executor's position — the adult's Stores tile, 15-20
tiles from the nursery — and the kitten was outside
`CARETAKE_RANGE = 12`, so `target = None`, step advanced vacuously.
The same silent-advance class as Phase 4c.3's original bug,
different surface. Fix: in `evaluate_and_plan`, after plan
creation, seed `plan.step_state[feed_idx].target_entity` from
`caretake_resolution.target` (captured at scoring time from the
adult's *original* position). Mirrors how `socialize_target` and
`mate_target` already flowed their resolver output into the plan
via `evaluate_and_plan` instead of asking the step executor to
re-resolve from a stale position. Caretake was the outlier.

**(4) Farming resurrected + canaries wired.** Collateral finding
while inspecting the footer's plan-failure histogram during
Caretake diagnosis — `450× TendCrops: no target for Tend` per
baseline soak, 0 harvests ever logged, no `Feature::*Crop*`
variant to catch it. Farming had been silently dead since it
shipped because `src/steps/building/construct.rs` only attached
`StoredItems` when `blueprint == StructureType::Stores` — Gardens
never got their `CropState` component, so the TendCrops
target-resolution query's `has_crop` filter was permanently false.
Fix: mirror the `Stores → StoredItems` special case one line up
(`blueprint == StructureType::Garden` → `CropState::default()`).
Added `Feature::CropTended` + `Feature::CropHarvested` (both
Positive) with wiring; `resolve_tend` return-shape adjusted so
the canary only fires when the tend math actually ran, not while
pathing toward the garden.

**(5) StepOutcome<W> global refactor (parallel work).** The
silent-advance class that produced bugs (2)-(4) motivated a
substrate-wide type-level fix: `src/steps/outcome.rs::StepOutcome<W>`
wraps `StepResult` with a witness parameter
(`<()>` / `<bool>` / `<Option<T>>`). `record_if_witnessed` only
exists on witness-carrying shapes; callers can no longer emit a
positive `Feature::*` based on `StepResult::Advance` alone. The
Phase 4c.4 bundle's `resolve_tend` and `resolve_feed_kitten`
migrations are the first conversions; the audit also added 8 new
silent-advance canaries — `FoodEaten`, `Socialized`,
`GroomedOther`, `MentoredCat`, `ThreatEngaged`,
`MaterialsDelivered`, `BuildingRepaired`, `CourtshipInteraction` —
each backing a subsystem that previously advanced silently when
its target was missing or its payload wasn't transferred.

**Seed-42 `--duration 900` deep-soak (two runs for variance)**
(`logs/phase4c4-alloparenting-a-v3/events-run{1,2}.jsonl`):

| Metric | Baseline (4c.3) | v3 Run 1 | v3 Run 2 | Direction |
|---|---|---|---|---|
| `deaths_by_cause.Starvation` | 1 | 3 | 2 | stable within variance |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | 0 | canary passes |
| `continuity_tallies.grooming` | 268 | 174 | 189 | noise band |
| `continuity_tallies.courtship` | 4 | 5 | 3 | noise |
| KittenBorn | 2 | 3 | 2 | stable |
| **`KittenFed` activations** | **0** | **55** | **10** | **hypothesis validated** |
| KittenMatured | 0 | 0 | 0 | still blocked (see below) |
| `CropTended` activations | — | 4,076 | 16,667 | farming lit |
| `CropHarvested` activations | — | 44 | 397 | farming lit |
| `positive_features_active` / total | 16/34 | 20/36 | 20/36 | broader |

**Hypothesis concordance — Reframe A:**

> Bond-weighted compassion ⇒ `KittenFed ≥ 1` AND Starvation
> stable AND at least one kitten reaches Juvenile.

Partial validation:
- **`KittenFed ≥ 1`: ✓** 55 and 10 respectively. Bond-boost makes
  non-parents (Mallow, Ivy, Birch, Nettle — Mocha's bonded
  friends) actually pick Caretake in softmax; the combined
  (1)+(2)+(3) fixes make the chain actually transfer food.
  Pre-fix soaks had these adults scoring Caretake 0.4-0.57 but
  either not winning softmax or planning a silently-broken chain.
- **Starvation stable: ✓** 1 → 2-3. Within noise band;
  Bevy parallel-scheduler variance dominates at this seed.
- **Kitten reaches Juvenile: ✗** 0 KittenMatured in both runs.
  The first-born kitten (4445v8 at tick 1261224 in run 1) had
  the most time (~70k ticks) and was fed 55 times, but didn't
  hit Juvenile before sim end. **Two candidate causes — both
  downstream of Phase 4c.4, to be investigated separately:**
  (a) growth-rate tuning may not produce a Juvenile in a 900s
  soak even with consistent feeding; `docs/systems/growth.md`
  tuning is unexplored. (b) the Reedkit-33 and Duskkit-7/74
  starvation deaths hint kittens can still out-hunger their
  feeders at the low end; milk-yield scaling (the 4c.3 landing
  entry's "Next concrete follow-on") is the literature-aligned
  next step.

**Generational continuity canary remains blocked.** Not a
Reframe A failure — A was scoped specifically to make any adult
feed any hungry kitten, and that succeeded. Reaching Juvenile is
one growth-tuning or milk-quality follow-on away, and needs its
own hypothesis + measurement cycle rather than more Caretake
mechanism. Filing as a new open-work entry.

**Farming first-soak-ever metrics.** First time in branch history
that CropTended/CropHarvested have non-zero counts. `ForageItem:
nothing found while foraging` dropped less than expected (713 →
726/554) because cats now ALSO tend crops instead of only
foraging, but per-cat food-income per tick rose enough to keep
FoodLevel at 22-46 (run 1) and 41-46 (run 2) throughout, vs
baseline which drained to 0 around tick 1317k. The Stores-empty
famine-cluster mode (pre-fix run 1's 8-adult-cluster starvation)
did not recur in either soak.

**Retracting Reframe B-elder's implementation path.** Entry #15's
Reframe A-vs-B sequencing said "if Reframe A fails the
generational canary, Reframe B's hypothesis is live." Partial
validation: A lit `KittenFed` but not `KittenMatured`. Reframe B
(elder-hearth babysit) doesn't help here — more adults feeding
kittens isn't the gap; the gap is downstream of feeding. Reframing
B as **deferred** pending the growth/milk-yield follow-on, not
escalated.

**Follow-ons filed as new open-work:**
- Growth-rate tuning investigation — do kittens need faster
  life-stage advancement to hit Juvenile in a 900s soak, or is
  the soak window too short regardless?
- Milk-yield / nursing-quality model (Phase 4c.3 landing's
  literature-anchored follow-on #1 — still load-bearing).
- Multi-seed sweep (99, 7, 2025, 314) to confirm KittenFed > 0
  generalizes beyond seed 42.
