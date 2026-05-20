---
id: 273
title: Caretake plans complete but KittenFed never fires — kitten starvation chronic
status: parked
cluster: life-cycle
orchestration: substrate-sensitive
initiative: [generational-continuity]
added: 2026-05-11
parked: 2026-05-11
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Original premise was empirically wrong (see
"Audit findings" below). Re-framed as the downstream surface of a
perception-accuracy ratchet; parked behind upstream detection work.
-->

## Why

Every recent seed-42 deep-soak ends with one kitten born and that
kitten starving to death. The ORIGINAL framing of this ticket
asserted `Feature::KittenFed` "stays at 0" for the starved kitten and
proposed a Caretake-plan completion defect as the cause. A 2026-05-11
audit (`.claude/plans/let-s-work-273-dig-enchanted-wirth.md`) ran the
numbers and found that framing is wrong. KittenFed fired **3 times**
for Maplekit-92 in `logs/tuned-42` (delta-verified in SystemActivation:
+2 at tick 1282500, +1 at tick 1292600). The pre-127 baseline shows 9
KittenFed events. The kitten-care substrate fires correctly — just
rarely. The actual cause is upstream: cat perception of the threat
environment misaligns with ground truth in ways that keep Patrol
elevated, crowding Caretake out of the L3 softmax. Caretake selection
share during the kitten's lifespan was 0.24% vs Patrol's 50.6%.

## Audit findings (2026-05-11)

The audit walked every perception scalar that feeds Patrol vs
Caretake scoring and compared each to ground-truth world state.
Six gaps surfaced; four share the same shape (point-in-time
perception of signals that should integrate over time):

1. **`memory_threat_seen_proximity_sum` has no temporal decay**
   (`src/ai/scoring.rs:1928`, `memory_proximity_sums()`). `ThreatSeen`
   events stay at their original strength forever in per-cat memory.
   Cedar carries 18 banishments at full strength through the whole
   run; safety-deficit perception is permanently elevated regardless
   of actual current threat. **This is the load-bearing inaccuracy.**
2. **Fox scent decays at a 10-day half-life intentionally for
   territorial-mark semantics** (`src/resources/sim_constants.rs:4240-4247`),
   but Patrol-class consumers need a faster signal. Ticket 228/256
   resolved by severing Patrol's read; cleaner fix is to split the
   construct — opened as **ticket 283**.
3. **`kitten_cry_perceived` has two discontinuities**:
   - Spatial: single-bucket sample, not range-summed (cats one tile
     outside the cry disc hear nothing) — addressed by 243/244.
   - Temporal: `KittenCryMap.clear()` per tick (`src/systems/growth.rs:162`),
     no onset/offset smoothing, no duration weighting. A 1-tick
     cessation immediately silences perceived urgency; sustained
     crying doesn't compound. Possibly in 243/244's scope; if not,
     follow-on needed.
4. **No `damage_recency` scalar** — cats can't distinguish "just got
   hit" from "old wound". Ticket 234.
5. **No colony-shared ambush history** — only per-cat ThreatSeen
   memory exists; cats outside hearing range never learn. Ticket 219.
6. **Non-pickup work DSEs ignore body-state perception** — ticket 233.

The audit also names the **temporal-integration doctrine** that ties
gaps 1, 3, 4, 5 together: perception scalars driving safety/urgency
DSEs must integrate over time, not sample point events. Opened as
**ticket 282** (design-doctrine note for `docs/systems/`).

## Current architecture (layer-walk audit)

Promoted via the 2026-05-11 audit. Substrate is functional;
inaccuracy is in the perception layer, not the kitten-care pipeline.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers (kitten) | `src/components/markers.rs::IsParentOfHungryKitten`, `src/systems/growth.rs:175` | Marker authors correctly; hunger threshold 0.5 fires the cry painting | `[verified-correct]` |
| L1 perception (cry) | `src/resources/kitten_cry_map.rs`, `src/systems/growth.rs:152` | Re-stamped per tick, sense-range disc; cats sample at exact tile | `[verified-discontinuous]` — spatial (single-bucket) AND temporal (no onset/offset smoothing) |
| L1 perception (threat memory) | `src/ai/scoring.rs:1928` | `ThreatSeen` events summed within memory_nearby_radius, no decay on strength | `[verified-stale]` — strength persists indefinitely |
| L1 perception (fox scent) | `src/systems/wildlife.rs:2383`, `src/resources/sim_constants.rs:4240-4247` | 10-day half-life — correct for territorial-mark consumers, wrong for Patrol | `[verified-stale-for-consumer]` — see ticket 283 |
| L1 perception (damage recency) | n/a | Scalar absent; `AcuteHealthAdrenalineFlee` reads steady-state health_deficit | `[verified-absent]` — see ticket 234 |
| L1 perception (recent ambush map) | n/a | Colony-wide decaying ambush map absent | `[verified-absent]` — see ticket 219 |
| L2 DSE (Caretake) | `src/ai/dses/caretake.rs:66-92` | 3-axis WeightedSum: kitten_urgency 0.45 + caretake_compassion 0.30 + is_parent_of_hungry_kitten 0.25. Cry lift at modifier layer, not DSE axis | `[verified-correct]` — eligibility intact (alloparenting allowed, no parent-only filter) |
| L2 DSE (Patrol) | `src/ai/dses/patrol.rs` | Reads `safety_deficit` (logistic), `boldness`, perimeter distance, conditional route_cost. **Does not** read any threat-presence scalar | `[verified-correct]` — safety-deficit-driven by design (228/256), but inputs are polluted by stale-memory ratchet above |
| L3 softmax | `src/ai/scoring.rs` (softmax composition) | Caretake 0.24% vs Patrol 50.6% share during kitten lifespan | `[verified-correct]` — softmax behaves correctly given the scores it receives |
| Action→Disposition | `src/components/disposition.rs:248-346` | `Action::Caretake → DispositionKind::Caretaking`; reverse `Caretaking → [Caretake]` | `[verified-correct]` |
| Plan template | `src/systems/disposition.rs:2757-2775` (`build_caretaking_chain`) | Chain: MoveTo(Store) → RetrieveAnyFoodFromStores → MoveTo(kitten) → FeedKitten | `[verified-correct]` |
| Completion proxy | `src/ai/dses/caretake.rs:117-122` | `GoalState{label:"kitten_fed", achieved: |_,_| false}` — completion via plan completion, not goal achievement | `[verified-correct]` (by design) |
| Resolver | `src/steps/disposition/feed_kitten.rs` | Requires ticks≥10 + target.is_some() + inventory.take_food().is_some(); emits `Feature::KittenFed` on witness | `[verified-correct]` — fired 3 times for Maplekit-92 |

## Findings + Disposition

**Findings:** Substrate works. KittenFed fires (3 times in tuned-42,
9 in pre-127 baseline). Insufficient *frequency* of feeding is the
defect surface; the cause is upstream perception inaccuracy that
keeps Patrol high (and Caretake low) regardless of actual threat
state. Adults DO get into range — Cedar reached the kitten's exact
tile (45,18) at tick 1280600; Mocha got within 1 tile at 1296300.
Range and perception path are not the failure. Softmax priority is.

**Disposition: park.** This ticket is downstream of the perception-
temporal-integration ratchet. Caretake-specific parameter tuning
(raising base score, lift weight) would just shift the L3 cliff
without solving the upstream cause and is the wrong lever per memory
`feedback_park_demographic_dependent_tuning`. The right work is the
detection-layer tickets:

- **Ticket 282** (newly opened) — temporal-integration doctrine
- **Ticket 283** (newly opened) — fox-scent split (gap #2)
- Ticket 219 — RecentAmbushMap with decay (gap #5)
- Ticket 234 — damage_recency (gap #4)
- Tickets 243/244 — audible cue substrate; scope-check for
  temporal-discontinuity sub-gap (gap #3)
- Ticket 233 — body-state subscription (gap #6)
- Also: add per-cat decay to `ThreatSeen` event strength (gap #1).
  May warrant its own ticket; could fold into 219's scope.

`blocked-by` is left empty: multiple potential unblockers, informal
parking. Reassess once any one of 219/234/243/244/283 lands.

## Verification (when unparked)

1. `just soak-trace 42 Cedar` AFTER one or more of the upstream
   tickets lands (Cedar is the saturating threat-memory cat).
2. Compare against tuned-42:
   - `memory_threat_seen_proximity_sum` trajectory during the
     1278400–1299298 window
   - Patrol L3 share for Cedar across the run
   - Caretake action count during kitten lifespan
3. Hypothesis: with memory decay + recent-fox-presence channel,
   Cedar's memory-threat sum during a 6k-tick lull falls by >50%,
   Patrol share drops below 40%, and Caretake fires ≥5× during a
   kitten lifespan.
4. `just frame-diff <baseline> <new>` to attribute the L3 shift to
   specific DSEs.

## Out of scope

- Caretake-specific parameter tuning (rejected — see Disposition).
- The cry-substrate authoring side (243/244's scope).
- Memory-system mechanics (per-cat episodic memory has its own
  ticket lineage — see 207 / 258).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **156** (done, ai-substrate, score 0.94 (cross-cluster)) — Kitten starvation localized at (38,22) post-154 cascade — non-parent adults can…
- ✓ landed **164** (done, ai-substrate, score 0.93 (cross-cluster)) — Seed-42 (38,22) kitten cohort starves despite KittenCryMap
- ✓ landed **187** (done, life-cycle, score 0.91) — Kittens starve in the post-184 soak — RetrieveFoodForKitten plan-fails dominate

<!-- linkages:end -->
## Log
- 2026-05-11: opened.
- 2026-05-11: parked. Perception-accuracy audit reframed the
  ticket — original premise (KittenFed never fires) is empirically
  wrong; actual count was 3 in tuned-42, 9 in pre-127. Real cause
  is the perception-temporal-integration ratchet driving Patrol
  share to 50.6% and crowding out Caretake (0.24%). Opened ticket
  282 (doctrine) and ticket 283 (fox-scent split) as the first
  two pieces of upstream work. Audit lives at
  `.claude/plans/let-s-work-273-dig-enchanted-wirth.md`.
- 2026-05-19: accuracy audit pass — parked status correct; layer-walk audit table comprehensive; file paths (growth.rs, scoring.rs, sim_constants.rs, ai/dses/caretake.rs, patrol.rs) verified; related landed tickets (156, 164, 187) confirmed.
- 2026-05-20: verification against `logs/afk-overnight-2026-05-19` (6h soak, sim day 1856, 24× canonical duration). Framing holds at scale: 22 starvations (20 of them kittens, 2 adults), 177k `HandoffItem: no recipient on disposition` plan-failures (rate 0.27/tick), kittens dying in den-clusters at (20–22, 19–25) and (29, 26) while adults are Cooking/GroomOthering/HerbcraftGathering at the colony cluster [19–20, 19–24]. Caretake/Handoff combined ≈ 6% of action snapshots in a 5k-tick window around Bramblekit-42's death (tick 1332614) — consistent with 273's "rare Caretake election" reading at scale. Range/perception path still not the failure: when Caretake fires, adults reach kittens. Parking remains correct. Sibling bugfix opened separately for the goap-path resolver substrate-stub defect surfaced during this audit (`snaps.kitten_snapshot` statically `Vec::new()` at `goap.rs:3825`, only writer is the initializer; HandoffItem fallback at `goap.rs:7322-7344` reads from this empty vec when `target_entity.is_none()`). That defect is downstream-but-orthogonal to 273's perception ratchet — it surfaces a subset of the 177k failure count even within the rare-Caretake-election regime 273 names.
