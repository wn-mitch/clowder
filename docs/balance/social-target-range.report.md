# social_target_range 10 → 25 — rejected at range=25

**Status:** iteration 1 rejected on unpredicted reproductive regression.
Sub-task remains open in `docs/open-work.md` follow-on #1.

## Context

From follow-on #1 in `docs/open-work.md`: Explore dominates action-time at
44–47% on seed-42 soaks, while several leisure actions (Mentor, Caretake,
Cook) never fire. The proposed root cause for the Explore dominance was a
"dispersion feedback loop" — Explore wins → cat walks to periphery →
`has_social_target` turns false → Explore wins again. Sub-task 1 of that
entry: broaden `social_target_range` from 10 (combat-adjacency sized) to
~20–30 (cat-perceptual sized).

## Hypothesis

> Cats perceive each other at sight + scent range well beyond 10 tiles. The
> current gate is sized for combat adjacency, not social awareness. A
> 25-tile gate better reflects perceptual biology, so `has_social_target`
> becomes true more often, enabling Socialize and Groom-other to compete
> against Explore more frequently.

`has_social_target` gates Socialize (`src/ai/scoring.rs:262`) and the
Groom-other branch (`src/ai/scoring.rs:289`). Mentor uses a separate
`mentoring_detection_range` and additional skill-gap gates, so Mentor was
explicitly flagged as a diagnostic (not a prediction).

## Implementation

One line. `src/resources/sim_constants.rs:1672`:

```diff
- social_target_range: 10,
+ social_target_range: 25,
```

Constants-hash diff between the two runs confirms exactly this one field
changed. Commit context: `0ba77d3` (post-v0.2.0 bookkeeping), clean working
copy for the baseline, dirty (one-line edit) for the treatment.

## Prediction

Per `docs/balance/social-target-range.predictions.json`. Summary:

| Metric | Direction | Magnitude |
| --- | --- | --- |
| Socialize action-time fraction | up | +30% |
| Groom action-time fraction | up | +30% |
| Explore action-time fraction | down | −10% |
| Mentor action-time fraction | flat (diagnostic) | n/a |
| Starvation deaths | flat (bar: ≤ baseline) | 0 |
| ShadowFoxAmbush deaths | flat | ±20% |
| Catches per cat per day | flat | ±10% |

## Observation

Both runs: seed 42, `--duration 900`, release build, commit `0ba77d3`.
Headers differ only on `disposition.social_target_range` (10 vs 25).
Baseline: `logs/tuned-42-baseline-social-target-range/`.
Treatment: `logs/tuned-42-treatment-social-target-range-25/`.

### Action-time fractions (via `CatSnapshot.current_action`)

| Action | Baseline | Treatment | Δ (pp) | Δ (%) |
| --- | ---: | ---: | ---: | ---: |
| Explore | 44.2% | 44.4% | +0.2 | +0% |
| Socialize | 17.7% | 19.3% | +1.6 | +9% |
| Forage | 13.3% | 14.2% | +0.9 | +7% |
| Hunt | 8.3% | 6.8% | −1.5 | −18% |
| PracticeMagic | 4.6% | 5.0% | +0.4 | +9% |
| Eat | 3.6% | 3.8% | +0.2 | +6% |
| Sleep | 3.4% | 3.5% | +0.1 | +3% |
| Groom | 0.63% | 0.34% | −0.3 | −46% |
| Herbcraft | 0.75% | 0.88% | +0.1 | +17% |
| Coordinate | 0.22% | 0.27% | 0 | +23% |
| Patrol | 0.47% | 0.27% | −0.2 | −42% |
| Idle | 2.0% | 0.48% | −1.5 | −76% |
| Mentor | 0 | 0 | 0 | — |
| Mate | 0.01% | 0 | 0 | — |

Totals: baseline 13,695 cat-snapshots (13,695 × 100 ticks = 1.37M cat-ticks);
treatment 15,740 cat-snapshots (1.57M cat-ticks, i.e. +15% more colony-tick
budget because fewer cats starved).

### Discrete event counts

| Event | Baseline | Treatment | Δ |
| --- | ---: | ---: | ---: |
| BuildingConstructed | 10 | 10 | 0 |
| CoordinatorElected | 8 | 4 | −50% |
| Death | 4 | 1 | −75% |
| KittenBorn | 4 | 1 | −75% |
| MatingOccurred | 6 | 2 | −67% |
| PreyKilled | 355 | 329 | −7% |
| WardPlaced | 421 | 482 | +14% |
| bonds_formed (final) | 34 | 19 | −44% |
| friends_count (final) | 16 | 10 | −37% |

### Final ColonyScore

| Field | Baseline | Treatment |
| --- | ---: | ---: |
| welfare | 0.546 | 0.503 |
| nourishment | 0.662 | 0.720 |
| happiness | 0.623 | 0.582 |
| living_cats | 8 | 8 |
| deaths_starvation | 4 | 1 |

## Concordance

| Claim | Direction | Magnitude | Verdict |
| --- | --- | --- | --- |
| Socialize up | ✓ | short (+9% vs +30% predicted) | direction correct, magnitude under |
| Groom up | ✗ | reversed (−46%) | **direction wrong** |
| Explore down | ambiguous | within noise | inconclusive |
| Mentor flat (diagnostic) | ✓ | 0 → 0 | consistent |
| Starvation ≤ baseline | ✓ | improved (4 → 1) | canary passes |
| ShadowFoxAmbush ≤ 5 | ✓ | 0 → 0 | canary passes |
| Wipeout | ✓ | 8 alive both | canary passes |

### Unpredicted regressions (>30%)

Per CLAUDE.md §Balance Methodology, drift > ±30% without a predicting
hypothesis is a bug, not a feature.

- **MatingOccurred: 6 → 2 (−67%)** — no hypothesis predicted this
- **KittenBorn: 4 → 1 (−75%)** — no hypothesis predicted this
- **bonds_formed: 34 → 19 (−44%)** — no hypothesis predicted this
- **friends_count (final): 16 → 10 (−37%)** — no hypothesis predicted this
- **Groom: −46%** — hypothesis predicted +30% (direction wrong)

## Proposed mechanism for the regression

Socialize and Mate compete in the same scoring layer; both use warmth and
(for Mate) partial sociability inputs. With `has_social_target` true on
more ticks, Socialize consistently out-scores Mate at the margin because:

- Mate requires `has_eligible_mate` (season + partner availability), a
  narrower gate than Socialize's `has_social_target`.
- Socialize has lower urgency thresholds; once it wins repeatedly, social
  need saturates and further Socialize scores drop — but by then the mating
  urgency window may have closed (seasonal, or partner moved away).

Groom-other being crowded out is consistent with the same mechanism:
Socialize and Groom-other share the `has_social_target` gate; Socialize
dominates when it wins. Self-Groom is down too, possibly because
self-grooming's level_suppression(1) gate interacts with the shifted
need-satisfaction landscape (warmth satisfied via Socialize → less urge to
self-groom).

The starvation improvement (4 → 1) is real but orthogonal to the stated
hypothesis. Candidate mechanism: clustered cats are closer to stores on
average (less time walking back from periphery after Explore), so eating
happens faster once triggered.

## Verdict: reject iteration 1 (range=25)

Canaries pass, but the reproductive-continuity regression (−67% mating,
−75% kittens) is exactly the kind of drift the methodology flags as
rejection territory. The hypothesis did not predict this, and the
verisimilitude story (wider perceptual range) does not argue for
suppressing mating.

## Proposed iteration 2

Two directions, not yet explored in this session:

1. **Smaller bump (range 15).** The Manhattan-area delta 10→15 is 221→481
   tiles (~2.2× vs 6× for 10→25). This may lift Socialize modestly without
   the mating-crowd-out. Predict Socialize +5-10%, Mate within ±20%.

2. **Instrument and diagnose the Mate regression.** Before further tuning,
   add telemetry for per-tick score distributions across Actions, so we can
   see *directly* whether Mate is losing to Socialize at the top-score
   margin, or losing to something else (e.g., Mate's own gates rejecting
   more often because cats are clustered in non-mating configurations).

Direction 2 is the more rigorous next step but requires code changes
beyond the scoring constant. Direction 1 is cheap to try but may produce
another "smaller version of the same regression."

## Follow-on threads

- `docs/open-work.md` follow-on #1 remains open. This iteration's findings
  are recorded here; sub-task 1 (broaden `social_target_range`) is not yet
  resolved, pending iteration 2.
- Sub-task 2 (Explore saturation curve) and sub-task 3 (strategist
  coordinator) of the same follow-on are independent of this finding.
- The Mate regression raises a new question: do other leisure actions
  (Groom-other, Mentor, Caretake) also compete in a shared score budget
  with reproductive actions? If so, any lift to leisure scoring risks the
  same regression. Instrumentation (direction 2) would answer this.

## Archived data

- `logs/tuned-42-baseline-social-target-range/` — seed 42, range=10, commit
  `0ba77d3`, 15-min soak. Restored as `logs/tuned-42/` (canonical).
- `logs/tuned-42-treatment-social-target-range-25/` — seed 42, range=25,
  commit `0ba77d3` with one-line edit. Not restored; kept for forensics.
- `docs/balance/social-target-range.predictions.json` — four-artifact
  predictions written before treatment.
