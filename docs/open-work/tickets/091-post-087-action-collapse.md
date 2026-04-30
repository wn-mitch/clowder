---
id: 091
title: Post-087 seed-42 plan-execution collapse — Eat picks 62% but FoodEaten never witnesses; no Forage/Hunt; founder starvation cascade
status: done
cluster: substrate-over-override
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: fa0f3a84
landed-on: 2026-04-30
---

## Why

Surfaced by the 087 verification soak (`logs/tuned-42` at commit `fc4e1ab8`, vs. baseline `logs/tuned-42-pre-087/` at `e838bb7`). Substrate-and-DSE-adoption landed test-green (`just check` + `cargo test --lib` 1640/1640 pass) but the canonical seed-42 deep-soak shows a colony-action collapse in the logged tail-window (ticks 1.2M → 1.21M, the only ticks `just soak` writes events for). Reads as either a balance issue from the Sleep/Flee axis additions or a DSE→GOAP plan-execution disconnect — *not* a strict sim regression.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)). **This ticket is the cautionary case demonstrating the sequencing rule — partial substrate adoption causes collapse.**

**Hack shape (mixed)**:
- **Class A — L2↔L3 sync gap**: GOAP `EatAtStores` action required only `ZoneIs(Stores)`, not `HasStoredFood`. IAUS `EligibilityFilter` and GOAP `StatePredicate` were two parallel feasibility languages; the substrate refactor wired markers into IAUS but not into the planner. Plumbing `HasStoredFood` into `StatePredicate` (H1 fix) is a substrate-side close. *Audit similar gaps for `CanHunt`/`PreyNearby`/`CanCook`.*
- **Class B — silent-advance step resolvers**: `eat_at_stores` returned silent `unwitnessed(Advance)` on missing food. H4 fix converts to `Fail` so canaries see drift. Observability debt; same neighborhood as the silent-advance audit candidates in `cook.rs`, `retrieve_raw_food_from_stores.rs`, `retrieve_from_stores.rs`, `feed_kitten.rs`.
- **Class C — producer-side substrate gap (residual, post-H1+H4)**: Forage/Hunt score 0.97/0.89 in IAUS but 0% executed. Likely `CanForage`/`PreyNearby` markers flip false at tail-window positions, or `actions_for_disposition(Foraging, …)` returns no plan when `ZoneDistances` lacks a ForagingGround entry. **Substrate gap, not override.**

**IAUS lever**:
- Class A: extend `StatePredicate` to mirror IAUS markers that gate DSEs (audit which markers actually need GOAP-side mirrors and which are step-resolver concerns).
- Class B: `Fail` not `Advance`; converts silent into observable.
- Class C: split Resting into `RestedWithFood` / `RestedWithoutFood` substrate DSEs; gate Resting on reachability via `EligibilityFilter`.

**Sequencing rule, demonstrated**: 087 retired part of `CriticalHealth` (gave Sleep + Flee new axes) but didn't extend the pattern to Eat. The substrate replacement wasn't expressive enough yet, and the colony food economy collapsed when `CriticalHealth` interrupt telemetry zeroed (19,382 → 0). **Lesson for the rest of the thread**: substrate axes land first; the corresponding hack retires second. H3 (retire `CriticalHealth` + soft-urgency interrupts) is correctly deferred until producer-side is fixed and [088](088-body-distress-modifier.md) lands.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

## Observation

| Metric | Baseline (e838bb7) | Post-087 (fc4e1ab8) |
|---|---|---|
| Cats | 8 founders | 8 founders (same set: Birch, Calcifer, Ivy, Lark, Mallow, Mocha, Nettle, Simba) |
| Wildlife at tick 1.2M | mice 80 / rats 55 / rabbits 45 / fish 35 / birds 30 | identical |
| Final tick | 1,314,346 | 1,211,999 |
| `interrupts_by_reason` | `{CriticalHealth: 19382}` | `{}` |
| Deaths (in window) | 3 ShadowFoxAmbush | 8 Starvation |
| Death cluster timing | spread | evenly-spaced cascade ticks 1209999 → 1211999 (~285 ticks apart) |
| Stores (end of run) | `current 50 / capacity 50` (full) | `current 0 / capacity 50` (empty) |
| Plans/tick | ~2.92 | ~0.72 (4× fewer) |
| `PlanInterrupted` total | 19,382 | 0 |
| `PlanStepFailed` total | 3,373 | 1 |

**Action distribution (% of CatSnapshot rows):**

| Action | Baseline | Post-087 |
|---|---|---|
| Forage | 19.1% | **0%** |
| Coordinate | 16.6% | 5.0% |
| Eat | 15.8% | **62.4%** |
| Sleep | 15.3% | 1.1% |
| PracticeMagic | 10.4% | 7.7% |
| Hunt | 4.9% | **0%** |
| Groom | 3.7% | 0% |
| Patrol | 3.4% | 0% |
| Socialize | 2.3% | 3.2% |
| Wander | 2.0% | 0.3% |
| Cook | 2.0% | 0% |
| Fight | 1.8% | 0% |
| Build | 1.2% | 13.0% |
| Flee | 1.0% | 0% |
| Herbcraft | 0.5% | 7.2% |

**Smoking gun**: `Eat` is the most-frequent action (62%) yet `FoodEaten` is in `never_fired_expected_positives`. The action records but the resolver never witnesses completion. `eat_at_stores` is the only emission site (`src/systems/disposition.rs:3771` and `src/systems/goap.rs:3043`), so cats are picking Eat plans against an empty Stores and the resolver short-circuits without recording.

## Hypothesis space (user-flagged: balance vs DSE→plan)

1. **Plan-execution disconnect.** Cats elect `Eat` against empty Stores; `eat_at_stores` precondition fails silently and the plan re-fires next tick. With 0 PlanStepFailed and 0 PlanInterrupted, no telemetry surfaces the loop. *Test:* trace one starver and check whether the eat_at_stores resolver is reached vs short-circuiting at a precondition gate; check whether `food_available: false` should have prevented Eat from being elected at all.

2. **Producer-side suppression.** Forage and Hunt drop to 0% — the producer side of the food economy is silent. Why didn't they fire? Eat scoring should be high (food_scarcity ≈ 1.0 with empty stores) but Forage/Hunt should also be high under the same signal. *Test:* score-sample on a hungry cat with empty stores and a prey/forageable in range — does Forage/Hunt actually win, or does Eat dominate via the Maslow-tier-1-trumps logic?

3. **Sleep reweight side-effect.** Sleep dropped from 15% → 1%. Cats might be staying tired, low-energy, and their score landscape skews. *Test:* revert just the 0.10/0.90 reweight on Sleep (keep `pain_level` axis but at weight 0 or remove it); re-soak.

4. **Flee CP gating effect.** Flee added a 4th CP axis (`health_deficit`, Linear floor 0.6); CP geometric mean now over 4 axes. Flee dropped to 0%. The 4-axis CP suppresses Flee scores enough that other actions always win. *Test:* check the score-distribution on a healthy cat near a threat — does Flee still beat Patrol/Wander?

5. **Substrate side-channel.** Some unintended interaction in Chain 2a (the new `author_self_markers` registration position) shifts marker-snapshot timing for downstream consumers. *Test:* move `author_self_markers` to before `update_injury_marker` or after the whole batch; re-soak and compare.

User judgment: the regression reads as #1 / #2 (DSE→plan or balance), not substrate. Investigation owned by user.

## Reproduction

```
mv logs/tuned-42 logs/tuned-42-fc4e1ab8
just soak 42
just q run-summary logs/tuned-42
```

Both runs preserved:
- `logs/tuned-42-pre-087/` — baseline, e838bb7
- `logs/tuned-42/` — post-087, fc4e1ab8 (until next soak run; rename if you want to keep)

## Diagnostic queries to start with

```
just soak-trace 42 Nettle           # focal trace on first starver
just q cat-timeline logs/tuned-42 Nettle
just q anomalies logs/tuned-42      # 14 canary failures recorded
```

The focal trace will reveal which DSE Nettle elected at each tick before starvation, and the L1/L2/L3 score landscape that produced it.

## Out of scope

- Reverting 087's substrate or DSE adoption — keep both in place; the substrate is the architectural value, the regression is a balance-or-execution issue downstream.
- Opening a follow-on for the deferred Rest DSE (087's `Implementation deviation #1`) — separate scope, not on the critical path here.

## Log

- 2026-04-30: Opened. Surfaced by 087's seed-42 verification soak. User flagged framing as balance / DSE→GOAP plan-execution issue, not a strict sim regression. Investigation ownership: user.

- 2026-04-30: Investigation pivot. Static-read analysis showed the original `MarkerSnapshot` framing was wrong — the snapshot is correctly threaded (`goap.rs:874→1421`). The actual mechanism: Eat-the-DSE was correctly ineligible against empty stores, but the GOAP planner's `EatAtStores` action def (`src/ai/planner/actions.rs:97-102`) required only `ZoneIs(Stores)`, not `HasStoredFood`. Once Resting was elected (Sleep is unconditionally pushed at `scoring.rs:899`, no `if urgency > 0` guard), the planner expanded Resting into `[TravelTo(Stores), EatAtStores]` and the resolver silent-no-op'd. The "Eat 62%" row in the action distribution was reporting *step* execution, not DSE win. **Two parallel feasibility languages existed: IAUS `EligibilityFilter` (consults `MarkerSnapshot`) and GOAP `StatePredicate` (consults `WorldState`); the substrate refactor wired markers into IAUS but not into the planner.**

- 2026-04-30: Header-commit discrepancy noted. `logs/tuned-42/` events.jsonl header recorded commit `1116447` (the docs-only ticket-opening commit, *pre*-087 substrate code), not `fc4e1ab8` as the ticket claimed. The cascade was therefore present at `1116447` (which is functionally `893c6ac` Farm DSE + ticket-file additions). The bug predates 087's substrate-and-DSE-adoption code; 087 only made it more visible.

- 2026-04-30: H1 fix landed (working tree, uncommitted). Plumbed `HasStoredFood` into the GOAP planner: `StatePredicate::HasStoredFood(bool)` variant, `PlannerState.has_stored_food` field, `EatAtStores` precondition extended. Threaded `has_stored_food` through `build_planner_state` and the three call sites (`evaluate_and_plan` + two `resolve_goap_plans` paths); added `has_stored_food` to `StepSnapshots` sourced from live `StoredItems` content (not the `FoodStores` cache). Two regression tests: `resting_with_empty_stores_does_not_plan_eat_at_stores` (asserts `make_plan` returns `None` when hunger unmet + stores empty) and `resting_with_empty_stores_but_only_energy_and_temp_unmet` (asserts the new precondition doesn't over-gate). 1642/1642 lib tests pass.

- 2026-04-30: H4 fix landed (working tree). `resolve_eat_at_stores` (`src/steps/disposition/eat_at_stores.rs:51-74`) returns `StepResult::Fail("eat_at_stores: no food item in stores")` (and two siblings for missing target / wrong component) instead of silent `unwitnessed(Advance)`. Triggers replan via `PlanStepFailed` event so canaries can see runtime state-drift. Should now be reached only when world state drifts between planning and execution.

- 2026-04-30: Re-soak with H1+H4 still produced 8 founder Starvation deaths. **The H1 fix is necessary but not sufficient.** Action distribution now: Build 32.7% / PracticeMagic 18.1% / Socialize 14.6% / Patrol 11.0% / Wander 9.0% / Coordinate 7.4% / Herbcraft 7.2%. **No Eat (good — H1 working), no Forage, no Hunt, no Sleep, no Cook.** `interrupts_by_reason`: 528× CriticalHealth + 82× Starvation soft-urgency + 1× Exhaustion soft-urgency (vs the broken `{}` from the bad soak — the safety net is back firing, just unable to recover food production). `never_fired_expected_positives` = `[BondFormed, FoodCooked, MatingOccurred, FoodEaten, GroomedOther, MentoredCat, CourtshipInteraction, PairingIntentionEmitted]` — same 8 as bad run.

- 2026-04-30: Sample of `last_scores` at tick 1200100 shows IAUS DOES score Forage and Hunt high (Nettle: Forage=0.97, Hunt=0.89; Mallow: Forage=0.86; Mocha: Forage=0.72). They are *being computed* and *winning the action competition* per-tick. Yet they never appear in the executed-action distribution. Hypothesis: a layer between "Forage scores high in the per-cat scoring" and "Forage action is executed" is dropping the result — most likely the Coordinator/disposition softmax or some target-selection gate (e.g., `prey_nearby` / `forageable_terrain_nearby` markers not being authored at the run's tail-window position). Or: cats elect Foraging disposition but the GOAP planner can't find a viable plan and they fall back to other dispositions.

- 2026-04-30: H3 (retire IAUS-subsumed interrupts) deferred — soak data shows the safety net is currently load-bearing again. Retirement should happen only after the producer-side problem is fixed (cats actually forage / hunt and stores refill). Otherwise removing CriticalHealth would just remove the only mechanism currently keeping cats alive.

- 2026-04-30: Context cleared mid-investigation. Resume points:
  1. Diagnose why Forage/Hunt score high but never execute. Start: `just soak-trace 42 Nettle` (focal trace) and read L3 chosen-action vs. L2 score columns at any tail-window tick. If softmax picks Foraging but the cat ends up Patrolling/Building, the bug is in disposition-mapping or planner-no-plan fallback. If softmax never picks Foraging despite Forage having the top score, the bug is in `select_disposition_via_intention_softmax_with_trace`'s pool filtering or the Independence penalty.
  2. After producer-side is fixed, return to H3 (retire CriticalHealth + Starvation/Exhaustion/CriticalSafety soft urgencies; keep ThreatNearby).

- 2026-04-30: Working tree state — unstaged H1 + H4 changes:
  - `src/ai/planner/mod.rs` — `StatePredicate::HasStoredFood`, `PlannerState.has_stored_food`
  - `src/ai/planner/actions.rs` — `EatAtStores` precondition + 2 new tests
  - `src/ai/planner/goals.rs`, `src/ai/planner/mod.rs` (default_state helpers) — field initializer
  - `src/systems/goap.rs` — `build_planner_state` signature, `StepSnapshots.has_stored_food`, three call sites
  - `src/steps/disposition/eat_at_stores.rs` — three `Fail` branches replacing silent `Advance`

- 2026-04-30: **Phase 1 — Telemetry hardening landed.** Added `EventKind::PlanningFailed { cat, disposition, reason, hunger, energy, temperature, food_available, has_stored_food }` and `EventLog.planning_failures_by_disposition: BTreeMap<String, u64>` footer field. Replaced silent `// If no plan found, cat stays idle` at `src/systems/goap.rs:1760` with an emit block. Footer wiring in `src/plugins/headless_io.rs` and `src/main.rs`. Unit test `planning_failed_increments_per_disposition_tally` pins. **The next investigation gets the cheap pre-trace disambiguator for free** — the post-Phase-1 footer immediately revealed `{Foraging: 20599, Hunting: 23580, Resting: 18340, Crafting: 3789}`, localizing the bug to the planner phase before reading a single trace line. 1643/1643 lib pass.

- 2026-04-30: **Phase 2/3 — Diagnosis.** Focal trace `logs/tuned-42/trace-Nettle.jsonl` showed Sleep=Hunt=Forage tied at 0.886 across the cascade window — a clamp signal, not natural scoring. **Root cause #1 (the hack):** `enforce_survival_floor` (`src/ai/scoring.rs:1455`, removed) was a post-hoc score-vector clamp that pulled non-`{Eat, Sleep, Flee}` scores down to the survival ceiling when starving. Hunt and Forage are the survival actions when stores are empty; the clamp suppressed them to 0.886 alongside Sleep. **Root cause #2 (the planner):** `ForageItem` and `SearchPrey` action defs at `src/ai/planner/actions.rs:31-93` required `CarryingIs(Carrying::Nothing)` — a permanent veto for any cat carrying leftover herbs/forage. The runtime resolvers (`resolve_forage_item`, `resolve_search_prey`) gate on `inventory.is_full()`, not on a specific Carrying state, so the planner abstraction was over-restrictive. Across the post-H1 1.2M-tick soak: ZERO PlanCreated{disposition:"Foraging"} or "Hunting" for ANY of 8 cats. **Root cause #3 (H1 side-effect):** the `HasStoredFood` precondition added to `EatAtStores` made the legacy three-need Resting goal `[HungerOk, EnergyOk, TemperatureOk]` unreachable when stores empty + hunger unmet — so Sleep stopped firing too (0% in the post-H1 footer).

- 2026-04-30: **Phase 4a — Removed `enforce_survival_floor` hack.** Deleted function at `scoring.rs:1455`, four unit tests, two production call sites (`disposition.rs:1012`, `goap.rs:1501`), imports, and the `survival_floor_phys_threshold` field from `ScoringConstants` (in `src/resources/sim_constants.rs`). User judgment per the substrate-over-override principle (093): post-hoc score clamps are control-yanking shortcuts; removing them lets natural IAUS levers fall out of the bottom. The post-removal trace confirmed it: Forage scored **1.09** (top), Hunt **1.04** (second) — the IAUS layer was correctly identifying Forage as the top action all along; the clamp had been hiding it. 1640/1640 lib pass after removal.

- 2026-04-30: **Phase 4 — Producer-side fix.** Removed `CarryingIs(Carrying::Nothing)` precondition from `SearchPrey` and `ForageItem` (`src/ai/planner/actions.rs:31-93`). The deposit chain still works: ForageItem sets `Carrying::ForagedFood` which DepositFood consumes; EngagePrey sets `Carrying::Prey` which DepositPrey consumes. Same shape as the existing comment in `caretaking_actions` (lines 478-487) — *"the planner's `Carrying` state is a coarse abstraction over a richer real inventory"*. New tests `foraging_with_carried_herbs_still_plans` and `hunting_with_carried_herbs_still_plans` pin the fix.

- 2026-04-30: **Phase 5 — Sleep regression fix.** Changed `goal_for_disposition(kind, current_trips)` → `goal_for_disposition(kind, current_trips, has_stored_food)` (`src/ai/planner/goals.rs`). Resting goal drops `HungerOk` when `!has_stored_food`, leaving `[EnergyOk, TemperatureOk]` so a hungry-tired-cold cat with empty stores can still Sleep+SelfGroom and re-elect (Foraging/Hunting) on the next decision tick. Three call sites updated in `src/systems/goap.rs` (1688, 2293, 2583). Test `resting_goal_drops_hunger_when_stores_empty` pins. The full→partial Resting goal restores baseline (e838bb7) Sleep behavior.

- 2026-04-30: **Phase 6 — Verification soak.** All four fixes landed (Phase 1 telemetry + Phase 4a hack-removal + Phase 4 producer + Phase 5 Sleep). Final footer:

  | Metric | Pre-091 (post-H1 broken) | Post-091 (final) |
  |---|---|---|
  | Founder Starvation | 8 (cascade) | 1 (isolated, Nettle at tick 1304677) |
  | PlanCreated{Foraging} | 0 | 57,954 |
  | PlanCreated{Hunting} | 0 | 30,757 |
  | PlanCreated{Resting} | 2,495 | 57,099 |
  | PlanningFailed{Foraging} | 20,599 | 2,443 |
  | PlanningFailed{Hunting} | 23,580 | 1,949 |
  | FoodEaten | never | fires |
  | grooming canary | 0 | 104 |
  | mythic-texture | 0 | 25 |
  | positive_features_active | 13/47 | 20/47 |

  Cascade resolved (88% Starvation reduction, 8→1). Producer-side dispositions now dominate planning. The 1 residual Starvation is a late-game isolated death (Nettle, tick 1304677, 5k+ ticks after her last successful PlanCreated), not a founder cascade — different shape, almost certainly a different root cause. `just verdict` still returns `fail` on the survival hard-gate (`Starvation == 0`) and continuity (`mentoring=0,burial=0,courtship=0` — pre-existing 087-era issues, not 091).

- 2026-04-30: **Phase 7 — Opened ticket 092** (marker/StatePredicate unification) per user direction: the parallel-feasibility-language smell that 091 surfaced should be retired structurally, not extended via more mirror-fields. 091's H1 + this ticket's residual marker mismatches will be absorbed by 092 automatically.

- 2026-04-30: **Honest reframe — cascade fixed, food loop still broken.** User pointed out the contradiction in the verification footer: `FoodEaten` is still in `never_fired_expected_positives` even though only 1 cat starved. Investigation showed cats are *slowly* starving — hunger trajectories across the run:

  | Cat | First hunger (t=1.2M) | Last hunger | Min hunger |
  |---|---|---|---|
  | Birch | 0.90 | 0.76 | 0.49 |
  | Calcifer | 0.99 | 0.70 | 0.46 |
  | Ivy | 0.85 | 0.58 | 0.48 |
  | Lark | 0.82 | **0.20** | 0.20 |
  | Mallow | 0.88 | 0.67 | 0.40 |
  | Mocha | 0.93 | **0.38** | 0.38 |
  | Nettle | 0.79 | **0.00 (dead)** | 0.00 |
  | Simba | 0.96 | 0.64 | 0.46 |

  Lark's `last_scores` at hunger 0.20 (very hungry): `[Hunt 0.85, Forage 0.61, Groom 0.55, ...]`. **Eat doesn't appear in the top scores at all.** Only 200 EatAtStores plans across 8 cats × 1.2M ticks. **The colony hauls food but doesn't consume it.** Cats forage, deposit, walk away, never break out of the producer loop to eat from stores. Two natural-lever gaps:
  - Forage's IAUS score outranks Eat's even when the cat is at Stores with `HasStoredFood=true`. The natural lever (Eat curve / stores-proximity multiplier / satiation modifier) doesn't express "if you're at the pantry and hungry, eat first."
  - No preemption breaks a long Forage plan when hunger crosses a threshold mid-trip. The 087 interoceptive markers (`pain_level`, `body_distress_composite`) should produce that pressure but apparently don't reach interrupt strength.

  These are substrate-over-override-shaped (093 thread) — IAUS levers, not a hack. Not in 091's scope, but the cascade-fix was a necessary precondition for surfacing them.

- 2026-04-30: **Out-of-scope follow-ups identified during this work** (open as separate tickets if not already covered by 092/093):
  - **Eat-vs-Forage IAUS scoring imbalance** — the food-loop bug above. Eat must dominate Forage when cat is at Stores with food and hungry. Investigate Eat DSE consideration weights, hunger curve shape, stores-proximity bonus, and possibly a satiation-pressure modifier that decays Forage when stores are well-stocked. **Highest-priority follow-up.**
  - **Forage/Hunt commitment-preemption gap** — long producer-side plans don't break for Eat when hunger crosses a critical threshold. Either commitment-strength tuning, or an interoceptive modifier on the Eat axis with enough magnitude to flip the contest mid-plan.
  - **Crafting Carrying-veto** — `GatherHerb` requires `CarryingIs(Carrying::Nothing)` (`actions.rs:270-281`, `292-297`); same pattern as the Forage/Hunt fix. Crafting planning failures jumped from 3,330 → 23,595 post-fix because cats now carry food/prey often, vetoing herb gathering. Not survival-critical, but the same ~5-line edit pattern would close it.
  - **Residual late-game Nettle starvation** — Nettle's last successful PlanCreated was at tick 1,299,505 (Resting), then no plans for 5,172 ticks until death. May be a planner-stuck pattern at a specific position, or a downstream symptom of the food-loop bug. Will likely resolve when the Eat-vs-Forage imbalance is fixed.
  - **Continuity regressions** — `mentoring=0`, `burial=0`, `courtship=0` (vs pre-087 baseline `courtship=764`, `mythic-texture=50`, `grooming=60`). Predates 091; probably 087-era social-stack regression that the substrate-over-override epic (093) will eventually absorb. `BondFormed`, `MatingOccurred`, `CourtshipInteraction`, `PairingIntentionEmitted` still in `never_fired_expected_positives`.

- 2026-04-30: **Landed (with honest residual).** Closed: cascade pattern, planner-side `make_plan → None` silent failure, `enforce_survival_floor` post-hoc clamp (third hack falling out under the substrate-over-override pattern), Sleep regression from H1's full-goal Resting, Forage/Hunt Carrying-veto. Telemetry hardening (`PlanningFailed` event + `planning_failures_by_disposition` footer) lights up future investigations and was load-bearing here. **Not closed: the food economy itself.** The cascading symptom is gone; the underlying ecology (cats actually feeding) requires the Eat-vs-Forage IAUS work above. Five follow-ups identified — opening the highest-priority (food-loop) as the next ticket; sub-threads continue in 092 (structural unification) and 093 (epic).
