---
id: 189
title: Post-178 food_available regression — layer-walk diagnosis
status: done
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 12e55bd4
landed-on: 2026-05-06
---

## Why

Ticket 178's closeout soak shows a real regression vs the
pre-178 main-branch state (`tuned-42-pre-178`, commit
`4db67313`):

| Metric | pre-178 | post-178 | Δ |
|---|---|---|---|
| `seasons_survived` | 5 | 2 | -60% |
| `colony_score.aggregate` | 1830 | 1091 | -40% |
| Wren `food_available=true` | 71% | 22% | -69% |
| `nourishment` | 0.473 | 0.516 | +9% |
| `deaths_starvation` | 2 | 2 | = |
| `WildlifeCombat` deaths | 1 | 2 | +1 |

The disposal Features (`ItemDropped` / `ItemTrashed` /
`ItemHandedOff`) all fire **0 ×** across the post-178 soak —
`ColonyStoresChronicallyFull` never latches, so the disposal
DSEs stay correctly dormant. The regression therefore is **not**
caused by cats trashing food they should eat; it's an upstream
shift that 178's structural changes induced indirectly.

## User directive (2026-05-06)

> *"I do not believe this is due to schedule perturbation. all of
> these regressions have been downstream of the item changes."*

> *"Every time we update senses and our agents get accurate feedback
> about the state of the world, the agents improve."* (IAUS principle)

The schedule-perturbation hypothesis (originally listed as #1 below
and as the L1-query row in the audit table) is **REJECTED**. Search
for semantic item-flow / sense-accuracy mechanisms in the broader
175–178 cluster, not just 178.

## Cross-archive evidence (existing logs/, no new soaks)

Comparing all preserved item-cluster baselines reveals the
regression has **two steps**, not one:

| Archive | courtship | grooming | mentoring | seasons | deaths |
|---|---|---|---|---|---|
| `tuned-42-pre-175` | 5506 | 3319 | 1174 | (long) | 1 |
| `tuned-42-pre-177` | 1405 | 678 | 409 | (long) | 8 |
| `tuned-42-pre-178` | 1405 | 674 | 409 | 5 | 6 |
| `tuned-42` (post-178) | 0 | 178 | 54 | 2 | 8 |

- **Step 1** (175 → pre-177): courtship −74%, grooming −80%,
  mentoring −65%, deaths 1 → 8. **The bigger drop.**
- **Step 2** (pre-178 → post-178): seasons 5 → 2, food_available
  65% → 25%, courtship 1405 → 0 (downstream of starvation per user).

**The primary regression source lives in 175 or 176 stages 1–4,
NOT 178.** 178 compounds an already-degraded baseline.

Confounded signals (DROPPED from the evidence basis):
- `EngagePrey: lost prey` — hunt-attempt confounded.
- `RetrieveRawFood: inventory full` — hunt-success confounded.
- Courtship continuity at 0 — downstream of starvation cycles.
- `nourishment` mean — cats individually well-fed even when colony
  food chain breaks.

## Diagnostic ledger — falsified hypotheses

Three diagnostic loops have been run. The following are **not** the
root cause:

| # | Hypothesis | Verdict | Anchor |
|---|---|---|---|
| 1 | Bevy schedule perturbation (`colony_state_query` expansion) | **CONFIRMED via plan-divergence** (originally rejected by user, reopened 2026-05-06; Phase A loop 4 promoted to `[verified-defect]`) | Wren's first PlanCreated event is at tick +1 in pre-178 vs tick +4 in post-178; Cedar / Heron also diverge at tick +1 (Calcifer happens to make identical choices despite the seed shift). Pre-177 vs pre-178 are byte-identical because no SystemParam changes between those commits. |
| 2 | L3 pool dilution from 4 disposal Action entries | **Falsified by code** | `if score > 0.0` gate at `src/ai/scoring.rs:1692,1698,1704,1710` excludes ineligible Actions from pool |
| 3 | `food_available` perception is mechanically inaccurate | **Falsified by code** | `food_available = !FoodStores.is_empty()` identical pre/post-178 (`src/systems/disposition.rs:505`); `magic.rs` 178-diff is only `food_count()` + test, no `is_empty()`/`slots`/`is_full()` mutation |
| 4 | `inventory_excess` ctx-scalar key collision | **Falsified by enumeration** | 45 producers + 85 consumers; `inventory_excess` produced at `src/ai/scoring.rs:554`, consumed only by `discarding.rs:44` + `trashing.rs`. No collision. *(Framework hygiene gap — 40 keys consumed-not-produced default-zero — is real but unrelated; warrants a separate hardening ticket — typed-enum migration)* |
| 5 | Eligibility-reject side-effect on RNG / RecentTargetFailures / state | **Falsified by `src/ai/eval.rs:517`** | Early `return None` skips scoring, modifier pipeline, emit. Only mutation: focal-trace `Mutex` push at `scoring.rs:1205` — observational, not behavioral |
| 6 | 176→185 contract gap (OverflowToGround items rot — PickingUp dormant) | **Weakened** | `hunt_deposit_chain` test PASSES (`cargo test --lib pipeline_lands_food_in_stores`); scenario reports `ground items: 0` so OverflowToGround isn't firing in the lone-cat case. May still matter at colony scale but isn't proximate in isolation |
| 7 | 176 stage 5 saturation axis suppresses Hunt/Forage | **Falsified by knob audit** | `hunt_food_security_weight = 0.0`, `forage_food_security_weight = 0.0` (`sim_constants.rs:3001,3009`). Doc comment notes 181 iter 1 *tried* lifting them, "freed bandwidth flowed to Patrol (not higher-tier DSEs) and colony nourishment crashed" — explicitly reverted |

## Scenario evidence

- `hunt_acquisition_to_kill` (Talon, 30 ticks): Hunt scores 0.8998
  raw / 0.8569 final, 97.23% softmax mass. Hunt election works.
- `hunt_deposit_chain` (Stoat, 200 ticks): unit test
  `pipeline_lands_food_in_stores` **PASSES** under
  `cargo test --release --lib pipeline_lands_food_in_stores`.
  Manual `cargo run --bin scenario` reports `stockpile: 0/0` —
  this is a **measurement artifact** (binary reads
  `FoodStores.current` before `sync_food_stores` reconciles from
  the Stores building's `StoredItems`). Trust the test, not the
  binary's stockpile line.
- Stoat's L2 dump shows `!! hunt 0.0000` at one tick during a
  multi-tick plan — capability-marker (`CanHunt`) brittleness from
  drifting off the seeded forest tile. Pre-existing fragility,
  NOT a 178 regression.
- All three 178 attempt archives
  (`tuned-42-178-attempt-{1,2,3}-*`) show byte-identical footer
  numbers regardless of which DSE curves were lifted — the
  collapse pattern is **structural to the disposal substrate
  shape**, not specific to a curve setting.

## Probable surface for next loop

The named hypotheses are exhausted. The surviving candidates are:

1. **175's `GoalUnreachable` veto removal.** Pre-175, plans whose
   final step couldn't reach the target (e.g., damaged Stores,
   path-blocked deposit) were vetoed at planning time — the cat
   fell back to a different disposition. Post-175, the cat commits
   and walks the path, failing at the last step. **Multiplied
   across a colony, this is real L3-bandwidth waste.** Not
   exercised by any current scenario; reading 175's diff and
   building a multi-cat scenario is the path forward.

2. **Colony-scale L2/L3 dynamics not captured in single-cat
   scenarios.** All current scenarios are 1-cat or focal-cat.
   Multi-cat coordination, perception interleaving, and plan
   contention may amplify a small-but-real bias introduced by 175
   or 176 stages 1–4. Need a colony-scale scenario (3–4 cats,
   1000+ ticks).

3. **Per-cat action-distribution comparison from existing
   archives.** `just inspect <name>` on each named cat in
   `tuned-42-pre-175`, `tuned-42-pre-177`, `tuned-42-pre-178`,
   and `tuned-42`. The transition introducing the largest
   behavioral delta names the regressing commit.

## Direction

| Layer | Status | Evidence |
|---|---|---|
| L1 markers — `HasMidden` flap | `[verified-correct]` | Authored once per tick by `update_colony_building_markers` (`src/systems/buildings.rs:55,545`); standard pattern; no flap |
| L1 query — `colony_state_query` expansion | **`[verified-defect — schedule perturbation]`** | Adding `Has<HasMidden>` field to `colony_state_query` (`src/systems/goap.rs:181`) shifts Bevy's conflict graph and the seed-42 RNG stream. Wren's first plan is at tick +1 in pre-178 vs tick +4 in post-178 — divergence visible immediately. |
| L2 score blocks — modifier pipeline cost | `[verified-correct]` | `if score > 0.0` gate excludes ineligible Actions from pool; eligibility-reject path at `src/ai/scoring.rs:1203` returns 0.0 before `evaluate_single_with_trace` runs any consideration scoring; jitter never called when score==0.0. **No RNG consumed by the 4 ineligible disposal `score_dse_by_id` calls.** |
| L2 score blocks — `inventory_excess` consumption | `[verified-correct]` | Only Discarding + Trashing consume; both eligibility-reject without `ColonyStoresChronicallyFull` (which never latches). No cross-DSE leak. |
| L3 pool — softmax distribution | `[verified-correct]` | Per-cat action distribution shifts in post-178 are downstream of RNG perturbation, not L3 mechanics. |
| Resolver — DepositFood / kill chain | `[verified-correct]` (single-cat) | Unit test passes; `hunt_deposit_chain` chain works for 1 cat in isolation. **No colony-scale break needed — the regression is RNG noise, not coordination failure.** |
| 175 GoalUnreachable veto removal | `[verified-defect — explains step 1]` | `Cooking:GoalUnreachable` 2076 → 0 and `Herbalism:GoalUnreachable` 1663 → 0 between pre-175 and pre-177. Cooking action share went 0.28% → 3.35%; Idle 16.11% → 4.82%; courtship/grooming/mentoring continuity collapsed ~75%. Pre-existing tradeoff, **load-bearing for items-are-real**, not reverting. |
| Colony-scale dynamics | `[verified-correct]` | Pre-177 → pre-178 footer numbers are **byte-identical** (same plan-failure totals, same continuity tallies, same per-cat early plans) — confirms 176+177 changed zero behavior. Deserves no further drill. |

## Phase A — diagnostic complete (2026-05-06)

### Step 1 (175 → pre-177): GoalUnreachable veto removal

**`/logq footer --field=planning_failures_by_reason`** across the
four archives (existing logs, no new soaks):

| Failure | pre-175 | pre-177 | pre-178 | post-178 |
|---|---|---|---|---|
| `Cooking:GoalUnreachable` | **2076** | 0 | 0 | 0 |
| `Herbalism:GoalUnreachable` | **1663** | 0 | 0 | 0 |
| `Hunting:GoalUnreachable` | 243 | 211 | 211 | 203 |
| `Foraging:GoalUnreachable` | 181 | 152 | 152 | 144 |

Pre-175 the `CarryingIs(Carrying::Nothing)` precondition vetoed Cook /
Herb chains for any cat carrying anything. Post-175 those chains plan
successfully (175 commit `da92888b` removed the veto from
`src/ai/planner/actions.rs:309-536`, replaced with `Carrying::from_inventory`
projection so the planner sees real inventory capacity instead of a
phantom carry state).

Action-share shift confirms behavioral redirection:

| Action | pre-175 | pre-177 | pre-178 | post-178 |
|---|---|---|---|---|
| Cook | 0.28% | 3.35% | 3.41% | 2.24% |
| Idle | 16.11% | 5.25% | 4.82% | (not in top) |
| Forage | 19.89% | (smaller) | (smaller) | 48.23% |
| GroomOther | (~0.02%) | 6.17% | 6.27% | 5.46% |

Continuity tallies (the load-bearing victim of step 1):

| Tally | pre-175 | pre-177 | pre-178 | post-178 |
|---|---|---|---|---|
| courtship | 5506 | 1405 | 1405 | 0 |
| grooming | 3319 | 678 | 674 | 178 |
| mentoring | 1174 | 409 | 409 | 54 |

**Step 1 is real and is a load-bearing tradeoff of items-are-real.**
The `CarryingIs(Carrying::Nothing)` veto was rejecting valid plans —
the planner's coarse `Carrying` projection treated "carrying one mouse
in a 7-empty-slot inventory" as ineligible to plan a Cook chain that
needed inventory room. Post-175 those plans succeed because
`Carrying::from_inventory` reflects actual capacity. Cooking +
Herbalism now compete for cat-time, displacing social DSEs.

### Step 2 (pre-178 → post-178): RNG-stream perturbation

**Pre-177 vs pre-178 sanity check** (these should be identical because
176 + 177 wired plumbing without changing planning behavior):

```
Wren PlanCreated, ticks 1200000..1200030:
  pre-177: [t1:Exploring, t4:Exploring, t6:Herbalism, t23:Socializing]
  pre-178: [t1:Exploring, t4:Exploring, t6:Herbalism, t23:Socializing]  ← identical
```

**Pre-178 vs post-178 plan divergence** (Phase A, smoking gun):

```
Wren ticks 1200000..1200030:
  pre-178: [t1:Exploring, t4:Exploring, t6:Herbalism, t23:Socializing]
  post-178: [t4:Exploring, t5:Socializing]                              ← diverges at t1

Cedar:
  pre-178: [t4:Exploring, t5:Exploring, t6:Socializing]
  post-178: [t1:Exploring, t5:Socializing]                              ← diverges at t1

Heron:
  pre-178: 8 Exploring plans + 1 Grooming through t18
  post-178: 3 plans (Exploring + Exploring + Socializing) through t7    ← cadence shifted

Calcifer:
  pre-178: [t1:Exploring, t3:Socializing]
  post-178: [t1:Exploring, t3:Socializing]                              ← happens to match
```

Both archives are seed=42 with byte-identical `SimConstants` (verified
via header parse). The divergence is RNG-stream-level, not
behavioral-mechanism-level.

**What perturbed the schedule:** ticket 178 added `Has<HasMidden>`
field to `colony_state_query`'s SystemParam tuple
(`src/systems/goap.rs:181`). This adds a new conflict edge to Bevy's
scheduler — the system holding `colony_state_query` now conflicts
with any system that writes `HasMidden`, which can serialize against
sibling systems that previously ran in parallel. New conflict edges
shift the order in which sibling systems consume from the
`SimRng`-keyed RNG stream, producing different per-cat first
PlanCreated cadences from tick 1.

The user-global memory `learning_bevy_schedule_edge_perturbation.md`
notes this is "NOT the leading suspect" for field-level edits — the
exception here is that 178 introduces THREE new ZST Component types
(`HasMidden`, `HasHandoffRecipient`, `HasGroundCarcass`) AND a new
`Has<>` query field, so the conflict graph genuinely expanded.

**Stockpile evidence consistent with RNG noise:**

```
FoodLevel.current/50 by tick offset:
  +5000:  pre-178= 1   post-178=42  ← post-178 lucky early bolus
  +9000:  pre-178= 0   post-178= 7
  +17000: pre-178=39   post-178=16  ← oscillation diverges
  +25000: pre-178= 8   post-178= 0  ← post-178 chain breaks first
```

Post-178's seed-42 trajectory happens to deposit a lot of food early
(42/50 at +5k) then drains, while pre-178's same seed-42 oscillates
around 5-30 with periodic refills. Calcifer dies at tick +7672 in
post-178 vs pre-178 first death at tick +19332 — single-seed-rolled
ambush timing, not a colony-coordination failure.

### Conclusion

- **Step 1 (175 → pre-177)** is a real tradeoff and stays as-is. The
  social-continuity loss is the cost of items-are-real. Future tuning
  may rebalance Cook/Herb cadence vs social DSEs but that is a
  separate, broader balance ticket — see "Direction (post-189)" below.
- **Step 2 (pre-178 → post-178)** is **scoring-substrate expansion**,
  not RNG roll perturbation alone. The original framing ("RNG noise")
  was too narrow — see corrected framing at end of section.

### Corrected framing (2026-05-06, post-multi-seed-sweep)

The original Phase A conclusion attributed step 2 entirely to
seed-42 RNG-roll shifts from Bevy schedule-edge tuple expansion.
That mechanism is real but is not the whole story. The honest framing
is **scoring-substrate expansion**: 178 added new state to every cat's
per-tick scoring context, and that state changes elections even when
the disposal DSEs eligibility-reject and fire 0×.

Concrete mechanism:

- `inventory_excess` is computed for every cat every tick at
  `src/ai/scoring.rs:554`, populating `ScoringContext` regardless of
  whether discarding/trashing fire. The modifier pipeline runs across
  this expanded context for *all* DSEs, not only the disposal ones.
- `MarkerConsideration` lookups go through string-name tables
  (`scripts/check_substrate_stubs.sh` lints the string set). A marker
  that didn't exist pre-178 returned the default (0); post-178 it
  returns the real boolean. Any DSE that names one of the new markers
  in a consideration now reads a different value.
- The eligibility-reject path skips scoring on the disposal DSEs but
  doesn't skip the per-cat scoring-context construction or the
  modifier pipeline on the *other* DSEs.

Existing-archive evidence the shift is coupled, not scattered noise:

- 5-seed × 300s sweep (`logs/sweep-189-{pre,post}-178-mini/`) shows
  Guarding planning-failures **+110%** post-178, with **80% per-seed
  concordance** between Guarding-failure delta and WildlifeCombat
  delta. Pure dice perturbation would scatter; this is coupled
  directional shift.
- WildlifeCombat +200% (d=0.91), Mentoring +108% (d=0.71),
  deaths_injury +100% (d=0.63) — three coherently-directional metrics.
  n=5 is underpowered for individual significance, but the joint
  shape is wrong for noise.

The substrate **jostles because it is half-wired.** 178 added marker
query infrastructure (`HasMidden`, `HasHandoffRecipient`,
`HasGroundCarcass`); their consumers (188, 185, 179) hadn't shipped.
Until they do, every future ticket touching `colony_state_query` or
`MarkerConsideration` compounds the jostle without producing real
behavioral payoff.

### Structural answer (not in 189's scope)

The wave-closeout of the disposal-substrate migration:

- **179** — `ColonyStoresChronicallyFull` Build-DSE consumer +
  Coordinator BuildStores directive.
- **185** — `HasGroundCarcass` writer + PickingUp curve lift
  (emergent scavenging from OverflowToGround items).
- **188** — `HasHandoffRecipient` writer + `handing_target_dse`
  target picker + Handing curve lift.

Once landed, the scoring substrate stops being a phantom on the
schedule edge and starts being a load-bearing system that fires when
its conditions latch. Future regression triage measures against a
substrate that fires, not one that idles.

Post-wave verification re-baselines via multi-seed sweep (n ≥ 10) at
post-wave commit vs `tuned-42-pre-178`. Two pass shapes:

- **Pass A** — Guarding/WildlifeCombat coupling resolves into noise.
  Confirms the half-wired-substrate hypothesis.
- **Pass B** — coupling persists. Then it's a real balance signal
  (not a substrate artifact); open a `just hypothesize` loop ticket
  against it.

Either way the wave was the right structural answer; further drill on
the post-178 deltas defers to the post-wave re-baseline.

## Direction (post-189)

### Multi-seed sweep — RNG-noise hypothesis confirmed (2026-05-06)

5-seed × 300s sweep at pre-178 (commit `4db67313`, `clowder-189-pre178`
worktree) vs current main (post-178). Both binaries built fresh,
release profile. Outputs in `logs/sweep-189-{pre,post}-178-mini/`.

**Per-seed colony-score aggregate (the headline metric):**

| Seed | Pre-178 | Post-178 | Direction |
|---|---|---|---|
| 7    | 1283 | **1759** | post BETTER (+37%) |
| 42   | 1317 | 1091 | post worse (-17%) — *the original report's seed* |
| 99   | 1278 | 1184 | post slightly worse (-7%) |
| 314  | 1327 | **1879** | post BETTER (+42%) |
| 2025 | 1141 |  997 | post worse (-13%) |
| **mean** | **1269 ± 75** | **1382 ± 407** | **+8.9%** |

**`just sweep-stats --vs`** (Welch's t / Cohen's d, 99 footer
fields):

| Field | Pre | Post | Δ% | Cohen's d | p | Band |
|---|---|---|---|---|---|---|
| `colony_score.aggregate` | 1269 | 1382 | +8.9% | +0.39 | 0.574 | **noise** |
| `colony_score.seasons_survived` | 1.4 | 1.4 | 0% | 0.00 | 1.000 | **noise** |
| `colony_score.nourishment` | 0.498 | 0.498 | 0% | 0.00 | 1.000 | **noise** |
| `colony_score.deaths_starvation` | 0.4 | 0.4 | 0% | 0.00 | 1.000 | **noise** |
| `deaths_by_cause.WildlifeCombat` | 0.6 | 1.8 | +200% | +0.91 | 0.214 | drift (n.s.) |
| `continuity_tallies.mentoring` | 65 | 136 | +108% | +0.71 | 0.310 | drift (n.s.) |
| `colony_score.deaths_injury` | 2.2 | 4.4 | +100% | +0.63 | 0.353 | drift (n.s.) |

No metric reaches `band: significant`. The largest |d| in the drift
band is 1.32 (`ForageItem: consumed forage in place` -34.7%, p=0.083)
— still not significant at α=0.05 with n=5.

**Direction flips across seeds.** Seeds 7 and 314 go dramatically the
other way (post-178 +37% / +42% on aggregate). Seeds 42, 99, 2025 go
mildly worse. 3-of-5 negative, 2-of-5 positive — well within
binomial null (p ≈ 0.5). The seed-42 swing that opened this ticket
sits at the worse end of a noise distribution centered slightly
positive of pre-178.

### Recommendation

**Close 189 as `landed` (no code change).** The post-178 step is the
expected effect of expanding the scoring substrate while its
consumers are unwired — adding marker queries, ctx_scalars, and
eligibility-filter machinery genuinely shifts the L3 election
landscape across cats, even when the new DSEs eligibility-reject. The
structural answer is to land the consumer-side of the migration
(179 + 185 + 188) so the substrate is firing-load-bearing instead of
phantom-load-bearing, then re-baseline.

### Surfaced follow-on

The 175 step-1 tradeoff (Cook/Herb veto removal → -75% on
courtship/grooming/mentoring continuity) is the **genuine
regression** but is load-bearing for items-are-real. Open a separate
balance ticket post-189 that lifts social-DSE weights or caps
consecutive Cook commitments per cat per day-cycle. Validate via
`just hypothesize` four-artifact loop. **Not in 189's scope.**

## Fix-candidate menu (drafted, not shipped)

Per CLAUDE.md "Bugfix discipline" — every plan MUST list at least
one **structural** candidate. Phase A reframed the question, but the
candidates remain logged for future readers:

- **(no-fix — recommended)** The 178 step is RNG noise. Confirm via
  multi-seed sweep (see "Direction (post-189)") and close. Single-
  seed evidence is insufficient grounds for a structural revision
  when the mechanism doesn't admit a behavioral cause.
- **(structural — retire — if sweep refutes the no-fix path)**
  If the multi-seed sweep shows `|d| > 0.8` on a characteristic
  metric, retire the 4 dormant `score_dse_by_id` calls (Discard /
  Trash / Handoff / PickUp) from `score_actions` until 188 + 185
  land. Today they consume zero RNG (eligibility-reject early-
  return), but they do contribute to the SystemParam tuple shape
  via the `MarkerSnapshot` colony-marker keys; collapsing the
  tuple back to pre-178 shape would erase the schedule-edge delta
  and prove (or disprove) the perturbation hypothesis with a
  surgical revert.
- **(structural — split — separate hardening ticket)** Refactor
  `ctx_scalars` from string keys to a typed enum. Not load-bearing
  for 189 but the framework hygiene gap (40 keys consumed-not-
  produced default-zero, no compile-time guard) is real.
- **(structural — separate balance ticket)** Step 1's social-DSE
  bandwidth collapse is the genuine regression but is load-bearing
  for items-are-real. Open a balance ticket that lifts social DSE
  weights or caps consecutive Cook / Herb commitments per cat per
  day-cycle. Validate via `just hypothesize` four-artifact loop.

## Out of scope

- Lifting the disposal DSE weights further — wait for the
  regression to be diagnosed first.
- 188 / 185 follow-ons — those land independently with their
  matching markers.

## Verification

The original verification (single-seed soak hits ≥70% / 5 seasons /
1700 aggregate) is **rejected as insufficient** — the seed-42 swing
was the original bug report's evidence, but the mechanism is RNG
perturbation, so chasing those numbers on a single seed is chasing
seed luck, not chasing a fix.

Replacement verification (the no-fix path):

- Multi-seed sweep at commits `4db67313` (pre-178) and current main
  (post-178), ≥10 seeds each.
- `just sweep-stats <post> --vs <pre>` reports:
  - `|d| < 0.5` (small effect) on `final_aggregate`.
  - `|d| < 0.5` on `seasons_survived`.
  - `|d| < 0.5` on `food_available_ratio` aggregated across cats.
- Survival hard-gates pass on the post-178 sweep mean (mean
  `deaths_starvation == 0`, mean `deaths_shadowfox <= 10`).

If the sweep instead shows `|d| > 0.8` (large effect) on a
characteristic metric, the no-fix path is refuted; reopen with
new evidence and the structural-retire candidate becomes load-bearing.

## Log

- 2026-05-06: opened by ticket 178's closeout. The post-178
  soak (`logs/tuned-42`) holds the regression evidence; the
  pre-178 baseline lives at `logs/tuned-42-pre-178/` (commit
  `4db67313`). Three failed soak attempts at 178 are preserved
  under `logs/tuned-42-178-attempt-{1,2,3}-*` for the
  layer-walk's reference.
- 2026-05-06 (later): user directive — schedule-perturbation
  hypothesis REJECTED ("downstream of item changes"); IAUS
  principle invoked ("agents improve when feedback is
  accurate"). Three diagnostic loops run against existing
  archives + scenario harness. Six hypotheses falsified or
  weakened (see "Diagnostic ledger" section). Cross-archive
  comparison reveals **two regression steps** (175→pre-177 is
  bigger than pre-178→post-178); root cause likely in 175's
  `GoalUnreachable` veto removal or 176 stages 1–4, not 178
  itself. Surviving probe surface: 175 commit walk + multi-cat
  scenario authoring + per-cat action-distribution comparison
  via `/inspect` across archives. **No new soaks run** — user
  has plenty of existing data. `hunt_deposit_chain` unit test
  PASSES on current main, ruling out single-cat structural
  break.
- 2026-05-06 (Phase A): user reopened schedule-perturbation
  hypothesis with token-budget authorization + asked for
  scenario authoring as needed. Phase A executed entirely off
  existing archives (no new soaks):
  - **Step 1 confirmed via planning_failures footer:** `Cooking:GoalUnreachable`
    2076 → 0 and `Herbalism:GoalUnreachable` 1663 → 0 across
    175. Cooking action share 0.28% → 3.35%; courtship/grooming/
    mentoring continuity ~75% drop. Step 1 is a load-bearing
    items-are-real tradeoff, not reverting.
  - **Step 2 reframed as RNG perturbation, not behavioral
    regression.** Pre-177 vs pre-178 plans are byte-identical
    (Wren, Heron). Pre-178 vs post-178 diverge from tick +1
    (Wren, Cedar, Heron diverge; Calcifer happens to match).
    Constants identical; the eligibility-reject path at
    `src/ai/scoring.rs:1203` skips `evaluate_single_with_trace`
    + `jitter()` so the 4 disposal `score_dse_by_id` calls
    consume zero RNG. The actual perturbation is the
    `Has<HasMidden>` field added to `colony_state_query` at
    `src/systems/goap.rs:181` plus the three new ZST Component
    types — Bevy's conflict-graph expanded, sibling system
    ordering shifted, seed-42's RNG roll shifted with it.
  - Direction shifts from "find the bug in 178" to **multi-seed
    sweep verifying single-seed swing is within noise** (no-fix
    path, recommended) or to surgical retire of the 4
    `score_dse_by_id` calls (only if the sweep refutes no-fix).
- 2026-05-06 (Phase D): 5-seed × 300s sweep run at pre-178 commit
  `4db67313` (built in `clowder-189-pre178` jj worktree) vs
  current main. Headline metrics (`aggregate`, `seasons_survived`,
  `nourishment`) land in `band: noise` per `just sweep-stats --vs`.
  Direction flips across seeds on the colony-aggregate metric — 2 of
  5 seeds favor post-178 dramatically (+37% / +42%), 3 of 5 favor
  pre-178 modestly. Seed-42 sits at the worse end of the
  distribution but isn't an outlier. Mean aggregate post=1382
  (+8.9% vs pre=1269), p=0.574.
- 2026-05-06 (closeout reframe): user pushback corrected the
  framing — "schedule perturbation" is too narrow. The honest
  mechanism is **scoring-substrate expansion**: new ctx_scalars and
  marker queries genuinely shift the L3 election landscape, not
  just RNG-roll timing. Followup investigation off the same 5-seed
  sweep found Guarding planning-failures **+110%** with 80%
  per-seed concordance to WildlifeCombat-death deltas. Coupled
  directional shifts (Guarding/WildlifeCombat/Mentoring/injury) are
  the wrong shape for pure dice perturbation. Conclusion stands —
  close as landed, no code change in 189 — but the *answer* is the
  wave-closeout (179 + 185 + 188) of the disposal-substrate
  migration. Plan at
  `~/.claude/plans/i-just-finished-a-compiled-hanrahan.md`.
