# Colony score: metric-effectiveness assessment (2026-06-09)

Companion to the TPS-invariant checkpoint landing. Question under
assessment: **is `ColonyScore.aggregate` an effective health metric for a
system like this** — a no-director, multi-equilibrium agent sim where the
gating tooling already carries hard survival gates and continuity
canaries?

Formula under assessment (`src/resources/colony_score.rs::aggregate`):

```
aggregate = welfare × max(1, seasons_survived)
          + achievement_points        (ledger: bonds/aspirations/structures/kittens ± death terms)
          + positive_activation_score (breadth/depth of positive Feature firings)
```

## Finding 1 — the dominant defect was measurement, not formula

Soaks run a fixed **wall-clock** budget (900 s). Both `welfare × seasons`
and the achievement ledger grow with elapsed **sim-time**, so end-of-run
aggregate conflates colony health with binary throughput. The 2026-05
perf regression (p90 197 → 72 t/s, epic 480) compressed observed seasons
~4.3 → ~3 and mechanically deflated aggregate ~30% with identical
per-tick behavior. The historical trajectory (best-ever 1330 post-085 →
800–1050 plateau) is therefore **partly a chart of binary speed**, and
score-chasing against it would have optimized the wrong thing — the
cheapest "score lever" on the books was literally the perf epic.

**Fix landed:** `colony_score_at_checkpoint` — snapshot AND ledger frozen
at `checkpoint_elapsed_ticks = 50_000` elapsed (2.5 seasons; 10k ticks
clear of any integer-season boundary; below the slowest current run's
~63k). `just verdict` prefers the checkpoint surface when both runs carry
it and labels the surface either way. **Post-checkpoint scores are a new
series — do not compare against the 1330-era numbers.**

## Finding 2 — residual weaknesses, ranked

1. **Point-in-time welfare.** Welfare is the last 100-tick emission, not
   an integral; a storm or mood dip at the capture tick swings the
   multiplier. Deterministic per seed, but cross-seed variance overstates
   behavioral variance.
2. **Integer-season multiplier.** Jumpy at boundaries (`max(1, seasons)`
   integer-clamps). The mid-season checkpoint mitigates; a continuous
   `elapsed/ticks_per_season` multiplier would fix it at the cost of
   series comparability — not worth it now (125's own "don't redefine
   the formula" boundary).
3. **Structurally blind to spatial defects.** The cuddle puddle (490)
   ran for a week with flat event tallies, unaffected `structures_built`,
   and an unmoved score while founders huddled at 4.7 tiles. No scalar
   reshaping fixes this class; the right instrument is a dedicated
   spatial canary (now wired: footer `founder_dispersion` + verdict
   floor).
4. **Gaming risk.** `positive_activation_score` rewards breadth/depth of
   positive-Feature firings (event-spam-shaped), and `kittens_weight=50`
   dwarfs the welfare scale. Any deliberate score-chasing must keep the
   hard canaries as the gate (125's substrate-over-override note stands).

## Verdict on effectiveness

**Effective as a continuous-health *lens*, ineffective as a *target*.**
For a system like this the score's real value is regression detection —
"all canaries green but aggregate moved 15%" — which is exactly how
verdict consumes it. As an optimization target it is Goodhart-prone
(weakness 4) and blind to the defect classes that actually matter here
(weakness 3). The colony's health is better understood as the **vector**
(welfare axes, continuity tallies, dispersion windows, throughput) than
the scalar; the scalar is the alarm, not the objective.

Recommendation adopted:
- Checkpoint score + existing canary suite is **sufficient** as the
  gating surface. No AUC / per-season-delta machinery in the sim — the
  per-tick `ColonyScore` event stream already supports computing those
  post-hoc in tooling (a script, not a sim change) if an investigation
  wants trajectory shape.
- Score-raising work should route through behavior (dispersion recovery
  → more forage/build/hunt → welfare axes), measured at the checkpoint,
  with mating/continuity canaries mandatory.
- Re-promote a baseline carrying the checkpoint block immediately, so
  the new series starts accumulating history.
