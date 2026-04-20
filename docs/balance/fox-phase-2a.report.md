# Fox Phase 2a — Circadian Disposition Bias

**Status:** landed with a significant downstream finding (see Concordance)

## Hypothesis

Foxes (Vulpes vulpes analogue) are crepuscular-nocturnal. Their GOAP disposition scoring should reflect this: Hunting dominates at Dusk and Night, Resting dominates during Day, Patrolling shows a mild crepuscular lean. Before Phase 2a, fox disposition scoring was phase-agnostic — the same hunger-driven urgency applied equally at Dawn, Day, Dusk, and Night.

## Implementation

- 12 new `ScoringConstants` fields (`fox_{hunt,patrol,rest}_{dawn,day,dusk,night}_bonus`) in `src/resources/sim_constants.rs` with serde defaults matching the doc's Phase 2 table.
- `FoxScoringContext` (`src/ai/fox_scoring.rs`) gains `day_phase: DayPhase` and `scoring: &'a ScoringConstants` fields.
- Additive phase offsets applied inside `score_fox_dispositions` at the Hunting / Patrolling / Resting scoring sites.
- `fox_evaluate_and_plan` (`src/systems/fox_goap.rs`) computes `DayPhase::from_tick(time.tick, &config)` and threads it plus the ScoringConstants reference through `build_scoring_context`.
- New `EventKind::FoxPlanCreated` variant in `src/resources/event_log.rs` emitted on every fox plan insertion — closes an observability gap the baseline soak exposed (Feature::Fox* emissions in `wildlife.rs` gate on non-planner entities, so the planner-driven foxes had no per-disposition signal before this PR).

Bonus values:

| Disposition | Dawn | Day  | Dusk | Night |
| ----------- | ---- | ---- | ---- | ----- |
| Hunting     | +0.3 | -0.2 | +0.5 | +0.7  |
| Patrolling  | +0.2 | -0.1 | +0.3 | +0.2  |
| Resting     |  0.0 | +0.5 |  0.0 |  0.0  |

## Prediction

Per `docs/balance/fox-phase-2a.predictions.json`.

## Observation

Post-change soak, seed 42, 15 min release build, commit 8d8fb85 (dirty):

**FoxPlanCreated distribution (n = 25,323 total):**

| Disposition | Count | Dawn | Day | Dusk | Night | Dusk+Night share | Day share |
| ---         |   ---:|  ---:| ---:|  ---:|   ---:|              ---:|       ---:|
| Avoiding    | 24,927 | 5,936 | 6,358 | 6,437 | 6,196 | 50.7% | 25.5% |
| Resting     |    383 |     4 |   378 |     0 |     1 |  0.3% | 98.7% |
| Patrolling  |     10 |     3 |     0 |     6 |     1 | 70.0% |  0.0% |
| Hunting     |      3 |     1 |     0 |     1 |     1 | 66.7% |  0.0% |

**Canaries (all pass):**
- `deaths_by_cause.Starvation`: 0
- `deaths_by_cause.ShadowFoxAmbush`: 4 (≤5 target)
- Colony survives; 0 Injury deaths
- Ambushes: 23 total (baseline 25, −8%) — distributed Dawn 7 / Day 4 / Dusk 7 / Night 5

**Parseability:** 30 of 55,749 event lines (0.05%) are corrupted by the log-writer's ring-buffer flush glitch. The jq-based `just check-canaries` recipe can't tolerate this; Python reads past the bad lines cleanly. See "Known issues" below.

## Concordance

| Metric | Prediction | Observed | Verdict |
| --- | --- | --- | --- |
| Resting Day share | up, ≥40% | **98.7%** | **concordant** (direction ✓; magnitude far exceeds prediction, but that's because Resting only wins against Avoiding in the specific high-hunger comfort windows where Day bonus pushes it over the threshold — not a bug) |
| Hunting {Dusk,Night} share | up, ≥60% | 66.7% (n=3) | **underpowered** — direction ✓ but sample size forbids a confidence claim |
| Patrolling {Dawn,Dusk,Night} share | up, ≥80% | 100% (n=10) | **underpowered** — direction ✓ but small sample |
| Total FoxPlanCreated | flat, ±20% | 25,323 (no prior baseline) | **unscorable** — pre-change baseline had zero FoxPlanCreated events (emission was added in this PR) |
| Starvation | 0 | 0 | **canary pass** |
| ShadowFoxAmbush | ≤5 | 4 | **canary pass** |
| Ambush total | ±20% | 23 (−8% from baseline 25) | **concordant** |
| Cat energy/mood p50 | ±5% | not re-measured | deferred — cat-side pipeline changes are out of scope |

**Overall verdict:** Phase 2a's circadian bias lands correctly at the scoring layer — the header confirms the bonus constants are active, and Resting's Day concentration is unambiguously driven by the new `fox_rest_day_bonus: +0.5`. Canaries pass.

**However, the observation exposes a pre-existing dominance bug that makes the Hunting/Patrolling circadian bias nearly invisible behaviorally:** 98.4% of all fox plans are `Avoiding`. With 10+ cats in the colony, any fox within the 6-tile cats_nearby range sees `cats_nearby ≥ 1`, and Avoiding's score `(cats_nearby as f32) * (1.0 - boldness * 0.8)` produces 1.0–2.0 — far above the Hunting/Patrolling scores Phase 2a biases. The bonuses fire correctly; the dispositions they bias just rarely win.

This finding is consistent with the baseline soak: zero Feature::FoxHuntedPrey / FoxScentMarked events in 15 min despite fox population of 4. Foxes weren't hunting pre-Phase-2a either. Phase 2a didn't cause this — it made the existing pathology observable for the first time.

## Next steps (not this PR)

1. **Fox Avoiding dominance — separate pull.** Options: raise the `cats_nearby ≥ 1` trigger to `≥ 2`, narrow the avoidance range from 6 to 3 tiles, or scale Avoiding by `1.0 / cat_count_colony_size` so foxes don't auto-Avoid in healthy colonies. This is a standalone scoring fix with its own hypothesis; don't bundle with Phase 2b.
2. **Log-writer flush corruption — separate pull.** `flush_event_entries` in `src/main.rs:919` produces occasional concatenated / partial records. Happens in both baseline and post-change runs (0.03–0.05% of lines). Investigate whether ring-buffer eviction races the flush, or whether multi-byte writes are interleaving.
3. **Phase 2b (shadowfox phase-gated spawn) unblocked.** The Phase 2a canaries passed, the bonus plumbing works. Phase 2b is independent — it targets `magic.rs::spawn_shadow_fox_from_corruption`, not fox scoring.

## Baseline reference

Pre-change baseline preserved at `logs/tuned-42-pre-fox-phase-2a/` for comparison. Key stats:
- 54,152 events, sim tick 1200000 → 1229021
- Colony survives; 2 Injury deaths, 0 Starvation, 0 ShadowFoxAmbush
- 25 Ambush events, 5 ShadowFoxSpawn events
- Zero FoxPlanCreated events (wiring added in this PR)
- Cat Resting plans: Night 33,778 vs. 4,928–5,942 per other phase — confirms Phase 1 crepuscular sleep is still active
