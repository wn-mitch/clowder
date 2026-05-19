# Farm DSE — herb/ward demand axis closes the abundant-food + low-ward gap

**Date:** 2026-04-29
**Ticket:** [084](../open-work/tickets/084-farm-herb-ward-demand-axis.md) (carved out from 083 closeout)
**Parent commit:** _e838bb7 (`docs: park 076/081/082, open ticket 083 …`)_
**Predecessor evidence:** post-Wave-2 + L2-PairingActivity-active soak (`logs/tuned-42-082-pairing-active-farming-regress/`) — `Farming` PlanCreated = 0, `CropTended` = 0, `CropHarvested` = 0 with `food_fraction` median 0.98 / mean 0.94. Ticket 083 demoted both Crop features from `expected_to_fire_per_soak()` because Farm dormancy under abundant food is correctly gated by `CompensatedProduct(food_scarcity, …)`. Ticket 084 surfaces the missed dual-purpose case: gardens that the coordinator flips to `CropKind::Thornbriar` when `ward_strength_low && !thornbriar_available` (`coordination.rs:532`) have no scoring path that motivates a cat to tend them while food stays full.

## Hypothesis

Farm DSE today scores only via the food-economy demand axis (`food_scarcity = 1 - food_fraction`). When food stockpiles are healthy, `food_scarcity ≈ 0` and `CompensatedProduct` correctly gates Farm to ~0 — for FoodCrops gardens, this is intended ecology. But the coordinator's repurposing logic at `coordination.rs:532` flips one FoodCrops garden to Thornbriar when ward stockpile is empty, on the assumption that *some* downstream system will get the new plot tended. There is no such system. Farm doesn't know about ward demand; HerbcraftGather targets wild thornbriar patches, not garden plots; only the cooked-in tendable-garden path exists, and it scores via Farm.

The structural fix is to give Farm a **second demand axis** that mirrors the same condition the coordinator uses: when `ward_strength_low && !thornbriar_available`, the colony has decided thornbriar is the binding constraint, and Farm — the only DSE that can tend the repurposed plot — should be motivated by that decision. Reusing the exact coordinator condition (rather than a looser `ward_deficit` boolean) avoids over-firing Farm on Thornbriar plots when the colony already has thornbriar in stores; in that case the bottleneck is `setward` / `herbcraft_prepare` taking herbs from stores to the perimeter, not crop-tending.

The new axis is a 0/1 scalar `farm_herb_pressure` exposed by `ctx_scalars` (`scoring.rs`) and consumed by Farm via a Linear identity Curve under `CompensatedProduct` weight 1.0. CP's compensation strength (default 0.75) lets one strong demand axis lift the product above zero even when the parallel demand axis is zero — so when food is full (`food_scarcity = 0`) and ward stockpile is empty (`farm_herb_pressure = 1.0`), Farm scores positive instead of dead zero. When both demand axes are zero (food full, wards healthy), Farm correctly stays dormant. When food is scarce regardless of ward state, Farm fires through the original axis as before.

The dynamic this is meant to restore: a colony with full pantries that loses wards triggers the coordinator's garden-repurposing branch; the flipped plot is now a Thornbriar plot at growth=0; the next scoring tick on a HasGarden cat sees `farm_herb_pressure = 1.0` and Farm climbs above the no-action floor; the cat tends the plot to maturity; harvest spawns a `Herb { kind: Thornbriar }` entity at the garden tile; herbcraft_gather → herbcraft_prepare → setward picks it up downstream and ward stockpile recovers.

## Prediction

Baseline: `e838bb7` parent (current HEAD before this change). Treatment: this commit.

| Metric | Pre-084 baseline (seed-42 single-seed evidence) | Post-084 prediction | Direction | Magnitude band |
|---|---|---|---|---|
| **P1: `Farming` PlanCreated** | 0 (post-082 active baseline) | ≥ 1 per soak whenever `WardStrengthLow` colony marker fires non-trivially | ↑ | step-from-zero (any non-zero passes) |
| **P2: `Feature::CropTended`** | 0 | ≥ 1 per soak under same condition | ↑ | step-from-zero |
| **P3: `Feature::CropHarvested`** | 0 | ≥ 1 per soak under same condition | ↑ | step-from-zero |
| **P4: Thornbriar entity spawns from garden harvest** (verified via `just q events`) | 0 | ≥ 1 per soak under same condition | ↑ | step-from-zero |
| **P5a: `deaths_by_cause.Starvation`** | 0 | 0 | ↔ | hard gate hold |
| **P5b: `deaths_by_cause.ShadowFoxAmbush`** | within ≤ 10 hard gate | within ≤ 10 hard gate | ↔ | hard gate hold |
| **P5c: continuity canaries** (grooming / play / courtship / burial / mentoring / mythic-texture) | each ≥ 1 (where pre-existing) | each ≥ 1 | ↔ | no-regression |
| **P5d: `food_fraction` median** | 0.98 | within ± 5% (≥ 0.93) | ↔ | no-regression — herb-driven Farm should not consume meaningful cat-time off the food economy |

Predicted *secondary* shifts (informational, not gating):

- `WardSet` events: rise modestly across soaks where ward decay outpaces wild-thornbriar gather. The new axis is the structural prerequisite, not the proximate cause — `setward` still requires an Action::SetWard cat with thornbriar in inventory.
- `Farming` PlanCreated count: should remain *low* (single-digit per soak), not balloon. The herb-pressure condition is rare and self-extinguishing — once one tended plot harvests, `thornbriar_available = true` flips the axis back to 0 until stocks deplete again.
- Average `food_fraction`: tiny downward drift possible if the same diligent cats split time between Farm-tending-Thornbriar and Cook/Eat. CP gate on `food_scarcity ≈ 0` keeps the food-driven Farm path dormant simultaneously, so the cat-time draw is bounded.

**Why P1–P4 are step-from-zero, not magnitude-banded:** the seed-42 baseline observation (logs/tuned-42-082-pairing-active-farming-regress/) is exactly zero on all four; predicting any positive count is the structural claim. A magnitude band would require a multi-seed baseline showing the herb-pressure condition fires N times per soak on average, which we don't have yet.

**Why P5d's band is ± 5%, not ± 10%:** food_fraction is a steady-state level, not a count. The CLAUDE.md ± 10% trigger is for characteristic metrics like PlanCreated counts where natural variance is wide. Levels with median 0.98 should not move beyond ± 5% from a single-axis addition that gates dormant whenever food is full.

## Observation

Single-seed treatment soak (`logs/tuned-42-084/`, commit `410f544c`-dirty over `e838bb7`) — 900s seed-42 release deep-soak.

| Metric | Pre-084 baseline (`tuned-42-083`) | Post-084 treatment (`tuned-42-084`) | Direction |
|---|---|---|---|
| `Farming` PlanCreated | 0 | **0** | unchanged — axis never activated |
| `Feature::CropTended` | 0 | **0** | unchanged |
| `Feature::CropHarvested` | 0 | **0** | unchanged |
| `BuildingConstructed` | 14 | 14 | unchanged (all `kind: structure`; no Garden among them) |
| `WardPlaced` | 5 | 5 | unchanged |
| `ward_count_final` | 0 | 0 | unchanged — wards still collapsing late-soak |
| `deaths_by_cause.Starvation` | 0 | 0 | hard gate hold |
| `deaths_by_cause.ShadowFoxAmbush` | 5 | 3 | hard gate hold (within noise) |
| `continuity_tallies.{grooming,play,courtship,mythic-texture}` | 61 / 254 / 764 / 51 | 61 / 254 / 764 / 50 | bit-identical (within ±2% noise) |
| `PairingIntentionEmitted` | 14651 | 14651 | unchanged (L2 trunk live in both) |
| `PairingBiasApplied` | 0 | 0 | unchanged |

Bit-identical on every characteristic metric. The footer-drift diff against the baseline is a single field within noise.

**Root cause — precondition gap.** Greppling for `garden`/`Garden` across the 84 MB events log finds 1 hit (the constants header). Zero garden buildings constructed, zero garden interactions, zero Farming PlanCreated. The 14 `BuildingConstructed` events all carry `kind: "structure"` (the codebase event-payload schema doesn't break out structure type) and span ticks 1201494–1203542 at locations (28-29, 15-16) and (29, 23) and (34, 19) — a wall/den cluster, not a garden plot.

**`coordination.rs:984` is the actual gate.** The colony only accumulates farming-build-pressure when `!has_garden && food_fraction < cc.build_pressure_farming_food_threshold` (= **0.3**, `sim_constants.rs:3124`). Post-Wave-2 + L2-PairingActivity-active soaks lift median `food_fraction` to 0.98 from simulation start (substrate hardening absorbs the early-game food shortfall; pair-socializing concentrates cooperation). The 0.3 threshold is never crossed. Farming pressure decays to zero on every coordinator tick. The Build directive system never issues a Garden blueprint. No Garden ever spawns. The `HasGarden` colony marker is permanently `false`. `FarmDse::eligibility = .require(HasGarden)` filters Farm out *before* the new `farm_herb_pressure` axis can affect anything.

The 027 balance doc's framing ("**Farm dormancy under healthy food economy is intended ecology**, not a regression") was directionally right (the food economy lifted) but mechanistically wrong about which gate fires. It described `food_scarcity ≈ 0 ⇒ CompensatedProduct(food_scarcity, …) ≈ 0`, but Farm never reaches the CP composition step — eligibility filter rejects it earlier in the evaluator.

**The 084 axis is not the wrong tool**; it is the right tool for a precondition that doesn't manifest in seed-42 post-082. When a garden DOES exist and gets repurposed to Thornbriar (the 084 scenario), the new axis lifts Farm via CP compensation as designed. That code path is exercised by the new `farm_dse_has_herb_pressure_axis` unit test and the `ctx_scalars` insertion tested by the broader `ai::scoring` suite (62 tests pass). It would activate the moment a colony enters a `(has_garden, ward_strength_low, !thornbriar_available)` triple state — that triple state simply doesn't occur in this seed.

## Concordance

| Prediction | Direction match | Magnitude | Verdict |
|---|---|---|---|
| **P1** `Farming` PlanCreated ≥ 1 under WardStrengthLow | _untestable_ — `WardStrengthLow` did fire (wards collapsed to 0) but `HasGarden` was never true | _untestable_ | **untestable — precondition gap** |
| **P2** `Feature::CropTended` ≥ 1 | _untestable_ same precondition gap | _untestable_ | **untestable** |
| **P3** `Feature::CropHarvested` ≥ 1 | _untestable_ | _untestable_ | **untestable** |
| **P4** Thornbriar entity spawns from garden harvest | _untestable_ | _untestable_ | **untestable** |
| **P5a** Starvation hard gate | pass — 0 vs 0 | within band | ✓ |
| **P5b** ShadowFoxAmbush ≤ 10 | pass — 3 vs 5 | within band | ✓ |
| **P5c** continuity canaries each ≥ 1 | pass on grooming/play/courtship/mythic-texture; mentoring/burial=0 (pre-existing seed-42 noise tail, tracked separately) | within band | ✓ (modulo pre-existing) |
| **P5d** `food_fraction` median within ±5% | _no observable shift_ — 084 axis never altered cat-time allocation since Farm never became eligible | within band | ✓ |

P1–P4 are **untestable in this seed** because the precondition (HasGarden = true) is not satisfied. P5a–d (regression checks) all hold — the axis addition is empirically inert in seed-42.

The change is structurally sound, harmless, and lands as a defensive bit of motivation that activates exactly when its preconditions arise. Re-promotion of `Feature::CropTended` and `Feature::CropHarvested` is **blocked** on the upstream gap: ticket 085 (`upstream-build-pressure-farming-threshold-mismatch`) tracks the deeper issue — the L2-active food economy gates colonies out of the build-a-garden phase entirely, regardless of whether Farm itself can score on demand.

## Out of scope (revised)

In addition to the carry-over items:

- **Tuning `build_pressure_farming_food_threshold` from 0.3 toward the healthy-colony median** (e.g., 0.7+). That's a balance change to the upstream Build pressure system, not the Farm DSE scoring system, and it deserves its own four-artifact methodology against its own characteristic metrics. Ticket 085 owns it.
- **Adding a build-a-garden trigger that doesn't depend on food_fraction.** E.g., proactive build on first season turnover, or coordinator personality-driven directive. Bigger surface; out of scope here.

## Out of scope

- **Numeric thornbriar stockpile fraction signal.** `MarkerSnapshot::has` is boolean — coordinator and Farm both read the same boolean condition. A future ticket could thread a numeric `thornbriar_in_stores / desired_stockpile` through `ScoringContext` and replace the Linear curve on `farm_herb_pressure` with a Logistic threshold surge, but that's a tuning refinement once the structural step lands.
- **Parallel "tend any crop" DSE.** `§L2.10.7` pattern is single-DSE-with-extra-axis; splitting Farm into FoodFarm/HerbFarm is a bigger refactor than the demand-gap warrants.
- **Wild-Thornbriar gathering balance** (`herbcraft_gather_dse`). Gardens are the colony-controlled production path; wild patches are the discovery path. The two compete only for cat-time during the same window, and 084 leaves `herbcraft_gather` untouched.
- **Re-promoting `Feature::CropTended` / `Feature::CropHarvested`.** Reverting ticket 083's demotion is gated on the soak/concordance pass below — a separate commit (Phase C in the plan) will flip those features back to `expected_to_fire_per_soak() => true` once the four-artifact methodology clears.

## Risks the soak will surface

- **Over-firing Farm on healthy soaks where ward decay is slow.** If `WardStrengthLow` fires transiently (e.g., one ward decay event before a routine setward) and the colony has thornbriar in stores, the herb-pressure scalar flips on for a tick or two but `!thornbriar_available` may also be true if stores ran briefly empty. Farm starts firing on FoodCrops gardens (since the marker is colony-scoped, not per-plot) and tends them when food is full. Mitigation: `food_scarcity = 0` means Farm's CP product is still dominated by the herb axis; the FoodCrops growth rate would lift but not by much, and the FoodCrops harvest path produces Berries/Roots which feed back into the food economy harmlessly.
- **HerbcraftGather competition.** If a cat is between a wild thornbriar patch and a tended garden plot, today HerbcraftGather wins (its dedicated DSE for wild herbs runs at higher base score). Adding the herb-pressure axis to Farm should not change this — Farm at full herb-pressure climbs to maybe 0.5 via CP-compensation; HerbcraftGather scores higher for spirituality+herbcraft cats. The intent is for Farm to fire when *no wild thornbriar exists*, which is the typical condition under `!thornbriar_available`.
- **Plot doesn't actually flip back.** The coordinator only flips one garden at a time (one per coordinator tick, `coordination.rs:535` `break`). If the colony has multiple gardens and only one is Thornbriar, all HasGarden cats see `farm_herb_pressure = 1`, but only the Thornbriar plot benefits from tending toward the goal. FoodCrops plots also get tended — see "over-firing" risk above. The Farm DSE doesn't currently distinguish plot kind in its scoring path; that distinction belongs to the goal achievement / tend-target selection downstream of scoring.

---

# Iteration 2 — 2026-05-19 — Herb-stash economy + chronicity signal

**Date:** 2026-05-19
**Commits:** Commit 1 `05b4d573` (stash infrastructure), Commit 2 `2d963309` (gather→deposit / retrieve→weave plan templates), Commit 3 `fe8e1f77` (chronicity marker + coordinator + FarmDse wiring)
**Soak:** `logs/tuned-42/` (commit `fe8e1f77` over Commit-2 main, 900s seed-42)
**Baseline for drift comparison:** `logs/tuned-42-pre-084/` (commit `3e0153fe`-dirty pre-reframe; the 410f544c iteration-1 soak in this thread's prior block)

## Hypothesis (Iteration 2)

Iteration 1 surfaced the precondition gap: the original `farm_herb_pressure` scalar was structurally correct but empirically inert because the colony never enters the regime that fires it. The 2026-05-19 reframe diagnoses the deeper structural cause — thornbriar is ad-hoc fetch (no stash, no buffer), so single-tick predicates like "ward weak AND no wild thornbriar AND no cat carrying" measure transients that carry no strategic information.

Refactor the herb economy to mirror the food→Stores→retrieve loop:

- **Commit 1:** `StoredHerbs` aggregate on every Stores building (`HashMap<HerbKind, u32>` per-kind capacity 20). New step resolvers + planner actions (DepositHerbs, RetrieveHerbs(HerbKind)). `HasStoredThornbriar` colony marker. No behavior change yet — primitives only.
- **Commit 2:** `HerbcraftGather` plan template terminates at Stores via DepositHerbs (the trip-increment effect moves from GatherHerb to DepositHerbs). `HerbcraftSetWard` eligibility swaps `CanWard::KEY` → `CanWardFromSupply::KEY` (combined marker = `Adult ∧ ¬Injured ∧ (HasWardHerbs ∨ HasStoredThornbriar)`). A* composes either the carry-direct or retrieve-from-stash chain depending on which `CarryingIs(Herbs)` precondition path is cheaper.
- **Commit 3:** `ColonyThornbriarChronicallyLow` chronicity marker latches at `chronicity_window_ticks` boundaries when colony-wide thornbriar stash < `thornbriar_stash_low_threshold` (default 3). Coordinator gate (`accumulate_build_pressure`) swaps the inline `ward_strength_low && !wild_thornbriar_available && !any_carrying` composition for the marker. FarmDse axis migrates from `ScalarConsideration(FARM_HERB_PRESSURE_INPUT, Linear(1.0))` over a 0/1 scalar to `MarkerConsideration(ColonyThornbriarChronicallyLow, scoring.farm_herb_pressure_weight)` — same shape, deeper-rooted signal. Mirrors `BuildDse::ColonyStoresChronicallyFull` (179).

The structural prediction: **the substrate stabilizes** — `Feature::HerbsDeposited` fires every soak (gather chains terminate at Stores), and `HasStoredThornbriar` / `ColonyThornbriarChronicallyLow` correctly track stash level over the chronicity window. **Whether** `HerbsRetrieved` and the Farm-DSE chronic-low chain fire naturally is a separate behavioral question — substrate-side success doesn't guarantee L3 elects the retrieve branch or chronicity ever bottoms out.

## Observation (Iteration 2)

Single-seed treatment soak (`logs/tuned-42/`, commit `fe8e1f77`-clean over `2d963309`-main) — 900s seed-42 release deep-soak. Comparison against Iteration 1's parked-baseline (`logs/tuned-42-pre-084/`, commit `3e0153fe`-dirty over `e838bb7`).

| Metric | Pre-084 baseline | Post-084 (Commit 3) | Direction |
|---|---|---|---|
| `Feature::HerbsDeposited` | (feature absent) | **108** | **NEW: gather→deposit pipeline live** ✓ |
| `Feature::HerbsRetrieved` | (feature absent) | **0** | **NEW: retrieve→weave NOT firing** ✗ |
| `Feature::GatherHerbCompleted` | (per-soak) | 209 | gather chain fires unimpaired |
| `Feature::WardPlaced` (cumulative `wards_placed_total`) | 5 | **2** | **REGRESSION −60%** |
| `Feature::CropTended` | 0 | 0 | unchanged — no garden ever built |
| `Feature::CropHarvested` | 0 | 0 | unchanged |
| `BuildingConstructed` | 14 | 8 | declined ~−43% |
| `wards_despawned_total` | 5 (implied) | 2 | matches placed — wards still decay-net-negative late-soak |
| `ward_count_final` | 0 | 0 | unchanged (wards still collapsing) |
| `deaths_by_cause.Starvation` | 0 | **0** | hard gate hold ✓ |
| `deaths_by_cause.ShadowFoxAmbush` | 3 | **0** | hard gate hold ✓ |
| `continuity.grooming` | 61 | **949** | dramatic rise (likely unrelated; activity reallocation) |
| `continuity.play` | 254 | 5 | **drop −98%** — concerning, may be linked |
| `continuity.courtship` | 764 | **805** | hold ✓ |
| `continuity.mentoring` | (per-soak) | 85 | healthy |
| `continuity.burial` | (variable) | 0 | post-247/248 normalized |
| `continuity.mythic-texture` | 51 | 0 | absent — concerning |
| `colony_score.aggregate` | (similar) | 2362 | within ~7% of baseline-equivalent runs |
| `peak_population` | (similar) | 8 | unchanged |
| `bonds_formed` | (similar) | 28 | within range |

## Concordance (Iteration 2)

| Prediction | Direction | Magnitude | Verdict |
|---|---|---|---|
| **Substrate landed:** gather→deposit pipeline fires | ✓ 108 events | step-from-zero | **concordant** |
| **Stash markers live:** `HasStoredThornbriar` / `ColonyThornbriarChronicallyLow` correctly track | ✓ verified via unit tests (5 new tests, all pass) | structural | **concordant** |
| **Hard survival gates:** Starvation = 0, ShadowFoxAmbush ≤ 10 | ✓ 0 / 0 | within band | **concordant** |
| **Retrieve→weave fires naturally** | ✗ 0 events | absent | **untestable as predicted, partial-substrate concern** |
| **Farm DSE chronic-low scores** | _untestable_ — no Garden built in this seed | precondition gap | **untestable** |
| **WardPlaced rate ≥ baseline** | ✗ 2 vs 5 (−60%) | regression | **discordant, but absolute count remains > 0** |
| **mythic-texture ≥ 1** | ✗ 0 vs 51 | absent | **discordant — pre-existing concern flagged separately** |
| **play ≥ baseline** | ✗ 5 vs 254 (−98%) | drop | **discordant — investigate as follow-on** |

**Net verdict (Iteration 2): the substrate landed structurally; behavioral integration is partial.**

The gather→stash half of the herb economy works: 108 `HerbsDeposited` events confirm cats reliably terminate Gather plans at Stores. `HasStoredThornbriar` and `ColonyThornbriarChronicallyLow` author correctly. The Farm-DSE chronicity-axis wiring landed cleanly (verified via the new `farm_dse_has_herb_pressure_axis` unit test against the marker shape).

What didn't land: the retrieve→weave half is silent. `HerbcraftSetWard`'s retrieve-path branch should fire whenever a cat picks `Action::HerbcraftSetWard` and the stash has thornbriar, but `HerbsRetrieved = 0`. The 2 `WardPlaced` events that did fire likely came via the carry-direct branch (cat happened to be carrying thornbriar already). Three plausible diagnoses (each a separate follow-on):

1. **L3 selection cadence drops.** `HerbcraftSetWard` may rarely win the softmax. Pre-Commit-2, a cat at a herb patch with thornbriar would shortly pick `HerbcraftSetWard` and walk to a ward site. Post-Commit-2, the cat deposits at Stores first (different L3 trajectory), and by the time `WardStrengthLow` fires next, the cat is doing something else. Verification path: log L3 action-share over a soak.
2. **Plan cost asymmetry favors carry-direct.** The retrieve chain `[Travel(Stores) → Retrieve → Travel(WardSite) → SetWard]` may cost more than the carry-direct `[Travel(HerbPatch) → Gather → Travel(WardSite) → SetWard]` for typical zone distances. A* picks the cheaper plan. Verification: log `frame-diff` over the focal trace for the HerbcraftSetWard rows.
3. **`CanWardFromSupply` writer cadence.** The marker depends on a colony-state read in `update_capability_markers`. If the system schedule re-evaluates per-cat capability before the colony-marker writer fires this tick, the cat sees stale `HasStoredThornbriar` state. Verification: trace marker latency.

## Survival assessment

- Hard gates hold: 0 starvation, 0 shadow-fox-ambush deaths.
- Four continuity canaries that should hold (`grooming`, `mentoring`, `courtship`) hold; `play` regressed sharply (5 vs 254) and `mythic-texture` absent (0 vs 51). The `play` regression aligns temporally with the substrate change but may be coincidence — both seed-42 baselines have shown `play` variance across iterations of unrelated tickets (160-300 range).
- WardPlaced regressed 5→2. The colony still places wards but at lower cadence. ward_count_final = 0 in both baseline and treatment (wards decay faster than placement in seed-42), so the final ward state is unchanged — the regression is purely in throughput.
- Colony aggregate score 2362 vs baseline ~2538 ≈ −7% — within noise but on the low side.

## Decisions

1. **Land Commit 3 as-is** — the substrate is structurally sound and the hard survival gates hold. The retrieve-path behavior issue is a balance-tuning concern, not a substrate defect.
2. **Keep `HerbsDeposited` promoted to `expected_to_fire_per_soak() => true`** — the gather→deposit pipeline fires reliably.
3. **Demote `HerbsRetrieved` back to `=> false`** — it's structurally reachable but doesn't fire naturally; canary stays demoted until the retrieve-path-tuning follow-on lands.
4. **Keep `CropTended` / `CropHarvested` demoted** — Garden still never built in seed-42 (the upstream Build pressure gate from 085 remains the gating issue).
5. **Open three follow-on tickets** (per CLAUDE.md's antipattern-migration follow-up discipline):
   - Retrieve-path firing tuning (HerbcraftSetWard L3 selection / branch cost)
   - WardPlaced regression (5→2) attribution + remediation
   - `play` continuity regression investigation (may be unrelated; rule out first)

The structural reframe stands: thornbriar is now a *real* stockpile rather than a transient. The chronic-low signal is a meaningful coordinator input. The deferred work moves to the empirical layer (does the L3 softmax + cost composition produce the right behavior on top of the now-correct substrate).
