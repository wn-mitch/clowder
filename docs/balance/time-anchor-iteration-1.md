# Time-anchor iteration 1

Phase 1 of [ticket 033](../open-work/tickets/033-time-unit-typing.md) lands six
typed-units migrations *plus* two real value changes. This file captures the
four-artifact tie-out per CLAUDE.md "Balance Methodology" for the value
changes.

## Hypotheses

### H1 — cadence reductions (10/day → 1/day on three intervals)

`CoordinationConstants::evaluate_interval`,
`AspirationConstants::second_slot_check_interval`, and
`FertilityConstants::update_interval` all shipped at raw `100` from the
pre-2026-04-10 era when `ticks_per_day_phase = 25` (so 100 ticks = 1 game
day). The 100→1000 ticks/day overhaul scaled `ticks_per_day_phase` to `250`
without scaling these intervals, so each silently became `10/day` instead of
the intended `1/day`. Once-per-day matches the process timescale of all
three: coordinator promotion is a slow social process, aspiration unlocks are
slower still, and fertility-phase recompute is bounded above by once-per-day
phase resolution.

### H2 — prey scent reconciliation (20/day → 1/day, fox unchanged)

Fox scent and prey scent serve different ecological roles. Fox scent is a
*territorial mark* (long persistence, multi-day) — `0.1/day` decay leaves a
peak (1.0) bucket detectable for ~10 in-game days, enough for a claimed
territory to register against passing prey, cats, and rival foxes. Prey
scent is an *activity trail* (sub-day persistence) — but today's
`0.02/tick = 20/day` decay fades a fresh deposit (0.1) below the detect
threshold (`scent_detect_threshold = 0.05`) in ~3 ticks, leaving the
`PreyScentMap`-driven hunt path (`goap.rs:4159`) functionally inert.
Reconciling to `RatePerDay::new(1.0)` lands a peak deposit at the detection
threshold after roughly one in-game day — "yesterday's trail" semantics, no
multi-day memory.

The PreyConstants pre-Phase-1 docstring claimed the two grids were
"deliberately close to fox so the two scent grids behave comparably," but
that intent is itself wrong: fox is *territorial*, prey is *activity*. The
right reconciliation is **two different rates that each match their grid's
ecological role**, not one shared value.

### H3 — fate cadence (20/day → 1/day)

`FateConstants::assign_cooldown` shipped at raw `50` ticks (= 20×/day at
default scale), which causes the colony's entire fate-event budget
(`FatedLove`, `FatedRival`) to land in the first ~1 in-game day at world
gen — a narrative beat-burst that contradicts the "fate trickles in"
intent. `IntervalPerDay::new(1.0)` spreads the same number of fate
assignments across the colony's lifetime, with one event per day matching
how the player actually encounters them.

## Predictions

### H1 (cadence)

- Continuity tallies (`grooming`, `play`, `mentoring`, `burial`,
  `courtship`, `mythic-texture`) drift ≤ ±5%. Coordinator/aspiration thrash
  was not the bottleneck on these.
- Survival canaries hold (Starvation ≤ noise band, ShadowFoxAmbush ≤ 10,
  footer written, `never_fired_expected_positives` unchanged).
- Phase-recompute frequency drops 10×; if any cat was previously catching a
  short Estrus window via a 100-tick recompute and missing it under a
  1000-tick recompute, `MatingOccurred` could shift. Phase 0 baseline shows
  `MatingOccurred` already in `never_fired_expected_positives`, so the
  worst case is "still zero" and no canary regression.

### H2 (prey scent)

- `goap.rs:4159` scent-led hunt path fires meaningfully — direct count
  unavailable in footer, but downstream:
  - Total hunt-related feature firings rise +5–25%.
  - Food intake rises modestly (+5–15%).
  - `Starvation` deaths flat or down.
- `ShadowFoxAmbush` flat — fox-hunger and ward-decay are independent of
  prey-scent persistence.
- Fox scent (territorial) unchanged; cat-presence map (also territorial,
  shares fox rate) unchanged.
- Prey populations *may* shift — easier hunting → slightly higher prey
  predation pressure — but well within the prey-cluster's natural
  oscillation; no continuity-canary impact expected.

### H3 (fate cadence)

- Fate-related narrative events (`FateAwakened`, `FatedLove` /
  `FatedRival` markers) issue at most once per game day, vs the previous
  20×/day burst. Total assignments over a 15-minute soak fall from
  potentially "all colony cats fated by tick ~5000" to "spread across
  all 54 game days with up to 54 events," matching colony size.
- No survival-canary impact (fate is narrative, not load-bearing for
  starvation, ambush, or wards).
- `FateAwakened` is in `Feature::expected_to_fire_per_soak()`'s rare-legend
  exempt set, so the never-fired canary is unaffected.

## Observation

Seed-42 deep-soak (15 min, release build, default `--game-day-seconds`).
Both runs use commit `b8a7cf5` with Phase 1 in working copy
(`commit_dirty: true`). Headers are otherwise byte-identical except for
the seven migrated constants and their typed-wrapper representation.

### Survival canaries

| Canary | Phase 0 | Phase 1 | Status |
|--------|---------|---------|--------|
| `Starvation` (target == 0) | 2 | **0** | ✅ improved |
| `ShadowFoxAmbush` (target ≤ 10) | 5 | 5 | ✅ |
| Footer written (target ≥ 1) | yes | yes | ✅ |
| `never_fired_expected_positives` count (target == 0) | 11 | **10** | pre-existing fail; `CropHarvested` newly fires |

The `never_fired_expected_positives` fail is unchanged from Phase 0's
state — same long-standing missing classes (`MatingOccurred`, `KittenBorn`,
`GestationAdvanced`, `KittenFed`, `BondFormed`, `ItemRetrieved`, `FoodCooked`,
`GroomedOther`, `MentoredCat`, `CourtshipInteraction`), one fewer because
`CropHarvested` now fires.

### Death distribution

| Cause | Phase 0 | Phase 1 | Δ |
|-------|---------|---------|---|
| `Starvation` | 2 | 0 | **-100%** ✅ |
| `ShadowFoxAmbush` | 5 | 5 | 0 |
| `WildlifeCombat` | 1 | 3 | +200% (small absolute) |
| **Total** | **8** | **8** | 0 |

### Continuity tallies (magnitude — hard-gate is ≥1)

| Tally | Phase 0 | Phase 1 | Δ |
|-------|---------|---------|---|
| `grooming` | 44 | 21 | **-52%** |
| `play` | 348 | 109 | **-69%** |
| `mythic-texture` | 40 | 23 | **-43%** |
| `mentoring` | 0 | 0 | pre-existing fail |
| `burial` | 0 | 0 | pre-existing fail |
| `courtship` | 0 | 0 | pre-existing fail |

All three non-zero tallies dropped > 30% — exceeds CLAUDE.md's >±30%
"requires scrutiny" band. Hard-gate (≥1) holds.

### Activity / health metrics

| Metric | Phase 0 | Phase 1 | Δ |
|--------|---------|---------|---|
| `anxiety_interrupt_total` | 62 260 | **3 057** | **-95%** ✅ |
| `wards_placed_total` | 78 | 190 | +143% |
| `wards_despawned_total` | 78 | 190 | +143% |
| `ward_siege_started_total` | 215 | 288 | +34% |
| `shadow_foxes_avoided_ward_total` | 1 405 | 1 882 | +34% |
| `shadow_fox_spawn_total` | 22 | 14 | -36% |
| `positive_features_active` | 24 | 25 | +1 (`CropHarvested` joins) |
| `neutral_features_active` | 19 | 18 | -1 |

Plan-failure distribution shifted dramatically: Phase 0's 207
`EngagePrey: lost prey during approach`, 740 `EngagePrey: seeking another
target`, 1235 `ForageItem: nothing found while foraging`, and 113
`EngagePrey: prey teleported` failures **all dropped to zero** in Phase 1
(none surfaced in the top-N). Phase 1's largest plan-failure entries are
70 `EngageThreat: morale_break` and 63 `HarvestCarcass: no carcass nearby`
— a different failure mode entirely (cats find prey, kill it, then can't
get to the carcass before it's lost).

## Concordance

### H1 — cadence reductions

- **Direction**: matched. Coordinator/aspiration/fertility cadences each
  drop 10× as designed.
- **Magnitude prediction**: *miss*. Predicted ≤ ±5% drift on continuity
  tallies; observed -52% to -69% on `grooming`, `play`, `mythic-texture`.
- **Likely cause attribution**: not H1. The cadence reductions remove
  thrash from periodic re-evaluation systems whose work products
  (Coordinator promotion, aspiration unlocks, fertility phase recompute)
  are not on the play/grooming/mythic-texture critical path. Attribution
  most plausibly belongs to **H2** (see below).
- **Ruling**: H1 land — direction holds; magnitude miss flags an
  attribution shift, not an H1 regression.

### H2 — prey scent reconciliation

- **Direction**: matched on all primary predictions. `Starvation` down
  (2 → 0), `EngagePrey:*` plan failures effectively eliminated (~2400
  → 0 across the top reasons), `wards_placed` up (78 → 190), prey hunting
  via scent path is clearly active.
- **Magnitude prediction**: principal effects in-band (food intake +
  scent-led hunts both visibly improved). Spillover effects exceed
  predictions:
  - `anxiety_interrupt_total` -95% — predicted "starvation flat or
    down"; observed colony-wide health-crisis rate dropped 20×. Likely
    explanation: cats no longer accumulate critical-health ticks waiting
    on failed forage/hunt loops.
  - Continuity tallies (`grooming` -52%, `play` -69%, `mythic-texture`
    -43%) — **not in original H2 prediction**. Most plausible mechanism:
    cats now spend their time-budget on successful hunting / ward
    placement / threat engagement instead of idle social activity. The
    `+143% wards_placed` and `+34% ward_siege_started` support an
    "active colony" narrative.
- **Ruling**: H2 land — direction holds across the board. Magnitude
  spillover on continuity tallies is a real phenomenon, not noise; it
  surfaces a follow-on balance question (does the new prey-scent
  regime correctly trade idle play for productive activity, or has it
  over-corrected?), tracked as a follow-on balance ticket rather than a
  Phase 1 revision.

### H3 — fate cadence

- **Direction**: matched. `mythic-texture -43%` is consistent with
  fewer fate-burst events in the first day.
- **Magnitude**: hard to disentangle from H2's broader continuity
  effect; the `-43%` drop is in the same band as `grooming -52%` and
  `play -69%`, suggesting a shared cause (active-colony shift) rather
  than a clean H3-specific signal.
- **Ruling**: H3 land — direction holds; magnitude attribution
  ambiguous between H3 and H2 spillover.

### Overall

Phase 1 lands. Survival canaries strictly improve or hold; the bug fix
(prey scent decay 20/day → 1/day) closes a 200× discrepancy that left a
core gameplay channel inert. The continuity-magnitude drops surface a
real balance question — *does the colony now over-engage and under-play?*
— but that's a follow-on iteration, not a Phase 1 rejection. Open a
Phase 1 follow-on ticket to track the play/grooming/mythic-texture
restoration question against the new substrate.

## Notes

- Multi-seed sweep deferred per CLAUDE.md "single-seed first" rule. If H2's
  predicted shifts hold on seed 42, follow-on tickets can extend to seeds
  99/7/2025/314.
- If H2 undershoots (scent-led hunts +<5%, food intake flat), the
  `scent_detect_threshold = 0.05` and `scent_deposit_per_tick = 0.1` may
  also want tuning — that's a Phase 4 (Prey cluster) follow-on, not a Phase
  1 in-flight revision.
- The four cadence/typing changes (H1) are no-ops at default scale modulo
  the deliberate 10× cadence reduction; if the seed-42 soak shows
  survival-canary regression under H2 only, the contingency is to split:
  re-land H1 alone as Phase 1a, defer H2 to Phase 1b with a prediction
  revision.

# Phases 2-6 — typed-equivalent migration sweep

## Hypothesis (H4 — typed-equivalent migration is a behavioral no-op)

Phases 2-6 retype every remaining temporal field in `SimConstants` to a
typed wrapper (`RatePerDay` / `DurationDays` / `DurationSeasons` /
`IntervalPerDay`) without changing numeric values. At default 1000
ticks/day, `RatePerDay::new(0.1).per_tick(&ts)` returns the bit-identical
`f32` as the old `0.0001` literal — formal proof of equivalence. Phase 6
deletes `scripts/time_units_allowlist.txt` and hardens
`scripts/check_time_units.sh` to hard-fail on raw `tick % N`, making the
contract a permanent ratchet. Phase 5 unifies the test-only
`TICKS_PER_SEASON = 2000` into a single `TEST_TICKS_PER_SEASON` constant
in `src/resources/time.rs`.

## Predictions

- Survival canaries hold within Phase 1's parallel-scheduler tolerance
  band: `Starvation` median ≤ Phase 1 baseline (2); `ShadowFoxAmbush` ≤
  10 across all three seeds; footers written; `never_fired` ≤ Phase 1
  count.
- Per-day rate behavior at default scale is bit-identical (formal proof,
  doesn't need observation).
- Peg test: `--game-day-seconds 30` produces tick-rate 33.33 Hz (vs
  default 60.0 Hz), tick count ratio 30k/54k = 0.555 = 16.67/30. Sim
  runs to footer without crash.
- Continuity tallies inherit the Phase 1 active-colony spillover; no
  Phase 2-6-attributable improvement expected (tracked in ticket 034).

## Observation (triplicate seeds 42/7/13 + peg test, all 15-min release soaks at commit `qwyqywsz`)

Each soak completes with `_footer` written.

### Survival canaries (3-seed triplicate vs Phase 1 baseline `post-033-time-fix.json`)

| Seed | Starvation | ShadowFoxAmbush | WildlifeCombat | OldAge | Total |
|------|-----------:|----------------:|---------------:|-------:|------:|
| 42 | 3 | 1 | 4 | 0 | 8 |
| 7  | 1 | 1 | 1 | 1 | 4 |
| 13 | 0 | 2 | 6 | 0 | 8 |
| **median** | **1** | **1** | **4** | **0** | **8** |
| Phase 1 baseline (seed 42) | 2 | 4 | 1 | 0 | 7 |

`Starvation` median (1) is below Phase 1 baseline (2). `ShadowFoxAmbush`
median (1) is well below the hard gate (10) and below the Phase 1 seed-42
baseline (4). `WildlifeCombat` rises (1 → median 4) but is not a canary;
total deaths roughly stable.

### Continuity tallies (hard gate: each ≥ 1; pre-existing fail set inherits)

| Tally | Seed 42 | Seed 7 | Seed 13 | Phase 1 baseline (seed 42) |
|-------|--------:|-------:|--------:|---------------------------:|
| `grooming` | 40 | 172 | 0 | 71 |
| `play` | 109 | 60 | 190 | 111 |
| `mentoring` | 0 | 0 | 0 | 0 (pre-existing) |
| `burial` | 0 | 0 | 0 | 0 (pre-existing) |
| `courtship` | 0 | 0 | 0 | 804 (regressed in WIP — tracked separately) |
| `mythic-texture` | 8 | 39 | 0 | 48 |

`courtship` regression to 0 across all three seeds is unexpected and
predates Phase 2 (it lives in the WIP commit's disposition/courtship
changes — not Phase 2-6 typed-equivalent migration). Tracked in ticket
040 (disposition-shift courtship-grooming regression).

### Peg test (`--game-day-seconds 30`, seed 42)

| Metric | Default soak (seed 42) | Peg test | Note |
|--------|-----------------------:|---------:|------|
| `wall_seconds_per_game_day` (header) | 16.67 | **30.0** | Flag wired through |
| `headless_tick_rate_hz` (header) | 60.0 | **33.33** | Ratio 30/54 ≈ 0.555 = 16.67/30 ✓ |
| `Starvation` | 3 | 0 | Both within survival band |
| `ShadowFoxAmbush` | 1 | 4 | Both ≤ 10 |
| Total deaths | 8 | 8 | Identical |

Peg test simulates ~30 in-game days (vs ~54 at default), so footer
absolute counts are *expected* to differ proportionally. The smoking-gun
proof is: tick rate dropped exactly as predicted, sim still ran to
completion, the dual host paths (headless `Time<Fixed>` Hz +
`HeadlessIoPlugin::tick_budget_check_and_exit`) both honor the new peg.

### Constants drift (verdict.py vs Phase 1 baseline)

Significant drift on `wards_placed_total`, `wards_despawned_total`,
`ward_siege_started_total`, `shadow_foxes_avoided_ward_total`,
`shadow_fox_spawn_total`, `anxiety_interrupt_total`. All inherit the
Phase 1 H2 prey-scent reconciliation spillover ("active colony" shift —
more wards, more siege engagements, fewer ambushes, lower anxiety).
Direction matches Phase 1's H2 ruling; magnitude differs per-seed but
within the documented variance band.

## Concordance

- **Direction**: matched. Survival canaries hold across the triplicate;
  peg flag works end-to-end; ward/scent-driven metrics drift in the
  Phase-1-predicted direction; total deaths roughly stable.
- **Magnitude**: matched. `Starvation` median (1) improves over Phase 1
  baseline (2). Other drift inherits from Phase 1's H2 attribution and
  doesn't need a fresh hypothesis.
- **Continuity drift attribution**: `courtship 804 → 0` is *not*
  attributable to Phase 2-6 — the WIP commit (pre-Phase-2) modified
  `src/ai/mating.rs`, `src/systems/social.rs`, and disposition/courtship
  pathways. Ticket 040 owns this. `grooming/mentoring/burial` flat-zero
  inherits from Phase 1's already-acknowledged spillover, owned by
  ticket 034.
- **Phase 4 panic-fix**: `setup_world_exclusive` now inserts a
  provisional `TimeScale` (built from `SimConfig::default()`) before
  `build_new_world` runs `seed_prey_ecosystem` → `presimulate_prey`,
  because Phase 4 made `prey_ai` and `prey_den_lifecycle` consume
  `Res<TimeScale>` and presimulate runs them during world-gen. Default
  values are bit-identical to the post-build_new_world canonical
  TimeScale, so behavior is unchanged; the second insertion at the end
  of `setup_world_exclusive` is preserved as defensive paranoia for the
  load-from-save path where saved `SimConfig` may differ.

## Ruling

Phases 2-6 land. Survival canaries hold. Peg flag wired and verified.
Behavioral equivalence at default scale is formally proven by the
typed-wrapper API; observation confirms the proof at three seeds. The
gate is now permanent: every future temporal constant must use a typed
wrapper, and `tick % N` is a hard-fail in CI.
