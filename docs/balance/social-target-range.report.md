# social_target_range — iteration 1 rejected, iteration 2 diagnostic

**Status:** sub-task 1 of follow-on #1 in `docs/open-work.md`. Range=25
rejected (iter 1). Instrumentation added (iter 2) reframes the regression
mechanism — not scoring competition, but **bond attenuation**.

## Iteration 1 — range=25 rejected (2026-04-19)

### Context

From follow-on #1 in `docs/open-work.md`: Explore dominates action-time at
44–47% on seed-42 soaks, while Mentor/Caretake/Cook never fire. The
proposed root cause: "dispersion feedback loop" — Explore wins → cat walks
to periphery → `has_social_target` turns false → Explore wins again.
Sub-task 1: broaden `social_target_range` 10 → 20–30.

### Hypothesis and change

`has_social_target` gates Socialize and Groom-other. Cat perceptual range
(sight + scent) exceeds 10 tiles; 25 better reflects biology. One-line
change: `src/resources/sim_constants.rs:1672`, 10 → 25.

### Observation (uninstrumented A/B at commit `0ba77d3`)

| Metric | Baseline | Treatment | Δ |
|---|---:|---:|---:|
| Socialize action-time | 17.7% | 19.3% | +9% |
| Groom action-time | 0.63% | 0.34% | −46% |
| Explore action-time | 44.2% | 44.4% | +0% |
| Starvation deaths | 4 | 1 | −75% |
| **MatingOccurred** | **6** | **2** | **−67%** |
| **KittenBorn** | **4** | **1** | **−75%** |
| **bonds_formed** | **34** | **19** | **−44%** |

Full table: `logs/tuned-42-baseline-social-target-range/` vs
`logs/tuned-42-treatment-social-target-range-25/`. Canary gates passed,
but unpredicted reproductive regressions >30% rejected the change per
CLAUDE.md §Balance Methodology.

### Initial hypothesis for the regression (rejected on iter 2 data)

First pass: Socialize and Mate compete for top score; with
`has_social_target` more often true, Socialize crowds Mate in the scoring
layer.

This turned out to be wrong — see iteration 2 below.

---

## Iteration 2 — full score-distribution instrumentation (2026-04-20)

### What changed

Commit `290a5d9`: `src/systems/goap.rs` and `src/systems/disposition.rs`
now log **every gate-open action score** (not just top 3) in
`CurrentAction.last_scores`, serialized into each `CatSnapshot` event. No
behavior change. Cap is the size of the Action enum (22). Analysis
script: `scripts/analyze_score_competition.py`.

### Instrumented A/B (seed 42, commit `290a5d9` with range override)

Clean constants-hash diff: only `disposition.social_target_range`
(10 vs 25).

#### Finding 1: Mate is gate-starved, not score-competed

| | Baseline | Treatment |
|---|---:|---:|
| Mate snapshots with gate open | 0 / 13,148 | 0 / 14,000-ish |
| Mate co-occurring with Socialize | 0 | 0 |

`has_eligible_mate` returns true on **zero** CatSnapshots in either run.
The gate opens only in brief windows between 100-tick samples. Baseline
had 4 MatingOccurred events; treatment had 0 — but neither case shows
Mate in `last_scores`, so the iter-1 "Socialize crowds Mate" hypothesis
is falsified at the scoring layer.

The gate requires, simultaneously, on both partners:
- Life stage Adult/Elder, non-Asexual, not pregnant
- Hunger > `breeding_hunger_floor` (0.6)
- Energy > `breeding_energy_floor` (0.5)
- Mood valence > `breeding_mood_floor` (0.2)
- Orientation-compatible
- Bond ∈ {Partners, Mates}

With 5 starvation deaths in the baseline, many cats dip below the hunger
floor regularly; co-occurring high-hunger-mood-energy windows are narrow.

#### Finding 2: Treatment regresses BOND PROGRESSION, not Mate scoring

| Final ColonyScore field | Baseline | Treatment | Δ |
|---|---:|---:|---:|
| bonds_formed (cumulative upgrades) | 35 | 17 | −51% |
| friends_count | 17 | 11 | −35% |
| partners_count | 0 | 0 | 0 |
| mates_count | 6 | 2 | −67% |
| MatingOccurred | 4 | 0 | −100% |
| KittenBorn | 5 | 0 | −100% |
| deaths_starvation | 5 | 0 | −100% |
| welfare | 0.530 | 0.525 | −1% |

Both runs start at mates_count=0; bonds progress during the soak. In
baseline, 3 pairs reach Mates (mates_count=6, 4 matings). In treatment, 1
pair reaches Mates (mates_count=2, 0 matings).

**Mechanism:** bond progression (Friends → Partners → Mates) requires
`fondness` and `familiarity` to build on a pair through repeated
interactions. Wider `social_target_range` means a cat's Socialize
interactions are spread across more candidate partners; each pair
accumulates fondness/familiarity more slowly; fewer pairs cross the
Partners and Mates thresholds. Reproduction collapses because the
`has_eligible_mate` requires a Partners/Mates bond and those bonds never
form.

#### Finding 3: Mentor gate-open dropped −17.8 pp

| | Baseline | Treatment | Δ |
|---|---:|---:|---:|
| Mentor gate-open | 43.7% | 25.8% | −17.8 pp |
| Mentor top-1 | 0.0% | 0.0% | 0 |
| Mentor mean score | 0.126 | 0.125 | ≈0 |

Unexpected: `has_mentoring_target` uses a separate constant
(`mentoring_detection_range`, also 10), not `social_target_range`. Yet
the gate dropped from 44% to 26% open. Likely mechanism: fewer skill-gap
pairs meet the threshold (mentor skill >0.6 ∧ apprentice skill <0.3)
because cats' skill-growth trajectories differed in the treatment's
different clustering pattern. Or: `observer_sees_at` vision interaction
with cat positioning. Worth further instrumentation before acting.

#### Finding 4: Mentor is score-starved, separately from its gate

Even in baseline where Mentor gate opens 43.7% of the time, Mentor wins
top-1 zero times. Mean score 0.126 vs Sleep 0.802, Eat 0.725, Hunt 0.669.

`mentor_warmth_diligence_scale = 0.5, mentor_ambition_bonus = 0.1`.
Compare to `socialize_sociability_scale = 2.0`: Mentor is 4× smaller in
magnitude and has stricter gating. Without a scale increase, Mentor
cannot win the scoring layer regardless of target availability.

#### Finding 5: Socialize wins via disposition rollup, not raw action

Socialize has 100% gate-open, 0.0% top-1, yet 17.7% action-time. The
disposition softmax rolls Socialize/Groom-other under Socializing
disposition; cats commit to the disposition for multiple ticks. Raw
action top-1 does not predict action-time for actions that live under
aggregating dispositions — useful context for future score analysis.

### Concordance — revised

| Claim | Direction | Magnitude | Verdict |
|---|---|---|---|
| Socialize gate-open up | flat | 100% → 100% | wasn't the lever |
| Socialize action-time up | ✓ | +9% (predicted +30%) | direction correct, magnitude under |
| Mate lost to Socialize in scoring | ✗ | Mate never scored at all | **original hypothesis wrong** |
| Starvation ≤ baseline | ✓ | 5 → 0 (canary improved) | |
| Wipeout | ✓ | both 8 alive | |
| MatingOccurred / KittenBorn | unpredicted regressions to 0 | | **bond-attenuation mechanism** |

### Revised conclusion

Sub-task 1 as written ("broaden `social_target_range` 10 → 20–30") is
**fundamentally compromised** by the bond-attenuation effect. A wider
range produces the intended Socialize lift but strips reproduction and
bond progression because fondness/familiarity builds on fewer pairs.

The real "dispersion feedback loop" described in follow-on #1 exists, but
the proposed lever (widening the social gate) is the wrong mechanism to
close it. The gate is already 100% open; the problem was never "cats can't
see social targets."

## Proposed iteration 3

Three directions, in order of how confident I am:

1. **Pair stickiness in social target selection.** When multiple cats are
   in `has_social_target` range, prefer the one with highest existing
   `fondness + familiarity`. This preserves bond-progression dynamics
   (pairs keep strengthening) while still allowing new introductions
   through long-fondness or random jitter. Touch points:
   `find_social_target` in `src/systems/disposition.rs:488` and
   `src/systems/goap.rs:742, 3796`. Small scoring-layer change with
   well-bounded blast radius.

2. **Investigate Mentor score magnitude** (separate from this sub-task).
   `mentor_warmth_diligence_scale = 0.5` is 4× below
   `socialize_sociability_scale = 2.0`, and Mentor has stricter gating.
   Raising the scale to ~1.5–2.0 should lift Mentor from 0% top-1 to
   non-zero. Predict 1–3 Mentor firings per seed-42 soak. Diagnostic
   canary: continuity-canary "mentoring fires ≥1× per soak" currently
   fails.

3. **Skip social-target range entirely; pursue sub-task 2** (Explore
   saturation curve from `docs/open-work.md` follow-on #1). That targets
   Explore's scoring formula rather than the social gate, so it doesn't
   risk bond attenuation. A saturation curve on
   `unexplored_nearby` past a local-familiarity threshold sharply drops
   Explore's weight without touching social dynamics.

Direction 1 is the cleanest continuation of this sub-task. Direction 2
is orthogonal and probably warrants its own thread. Direction 3 was
always next up in the open-work.md ordering.

## Artifacts (archived)

- `logs/tuned-42-baseline-social-target-range/` — pre-instrumentation
  baseline (top-3 scores only). Commit `0ba77d3`.
- `logs/tuned-42-treatment-social-target-range-25/` — pre-instrumentation
  treatment (top-3 scores only). Commit `0ba77d3`.
- `logs/tuned-42-baseline-instrumented/` — instrumented baseline. Commit
  `290a5d9`, clean. **Canonical baseline as of 2026-04-20.**
- `logs/tuned-42-treatment-instrumented/` — instrumented treatment
  (range=25). Commit `290a5d9` + one-line dirty edit.
- `docs/balance/social-target-range.predictions.json` — iter-1
  predictions, superseded but kept for provenance.
- `scripts/analyze_score_competition.py` — per-action score analysis.
  Usage: `scripts/analyze_score_competition.py --compare <base> <treat>`.
