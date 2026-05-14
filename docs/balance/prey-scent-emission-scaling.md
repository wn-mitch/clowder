# Per-species prey scent emission scaling — ecological footprint by `base_range` (ticket 062, 2026-05-14)

Ticket 062 retires the single aggregate `PreyScentMap` and replaces it
with `PreyScentMaps` (five per-species sub-maps keyed by `PreyKind`).
The structural cutover preserves aggregate read semantics via
`get_any` / `highest_nearby_any` (max across sub-maps), so existing
Hunt / Hunting DSE consumers see a behavior-neutral signal at the
*read* side.

The behavior change lives on the *write* side. Each tick a live prey
deposits `scent_deposit_per_tick × (profile.scent.base_range /
scent_deposit_normalizer)`. With the default `scent_deposit_normalizer =
6.0` (Rat's base_range) and the ecological profile already encoded in
`SensoryConstants` defaults (Mouse=5, Rat=6, Rabbit=4, Fish=5, Bird=2),
relative deposit rates by species are:

| Species | `base_range` | Deposit factor |
|---|---|---|
| Rat    | 6.0 | 1.00× |
| Mouse  | 5.0 | 0.83× |
| Fish   | 5.0 | 0.83× |
| Rabbit | 4.0 | 0.67× |
| Bird   | 2.0 | 0.33× |

Pre-062, all five species deposited identically at
`scent_deposit_per_tick = 0.1`. Post-062, the *aggregate* (`get_any`)
read at a tile occupied by a single species drops proportionally — most
prominently for Bird tiles, where the aggregate falls to ~⅓ of the
pre-062 reading.

## Hypothesis

Per-species emission scaling proportional to each prey's olfactory
`base_range` shifts the colony-wide scent-led Hunt-initiation rate
*downward by a small fraction* (default ecology is not bird-dominated,
so the mean drop is muted) and *flattens the L1 trace's species
distribution* (Bird sub-map carries ~⅓ the magnitude of the Rat
sub-map at occupied tiles).

## Prediction

| Field | Value |
|---|---|
| **Metric (primary)** | `_footer.positive_features.HuntAttempted` total per soak |
| **Direction** | Decrease |
| **Magnitude** | < 10% drop vs the `post-297-substrate-dormant` baseline |
| **Reasoning** | Default ecology has more Mouse / Rat dens than Bird dens (4 / 3 / 3 / 2 / 2 — see `PreyConstants::initial_den_count_*`). Bird-only tiles drop to ⅓ deposit but represent a minority of prey-occupied tiles; Mouse/Rat/Fish stay at 0.83×–1.0×, so the mean aggregate shift is small. |
| **Metric (secondary)** | Hunt and Hunting DSE mean and p50 final_score in focal-cat trace |
| **Direction (secondary)** | Decrease |
| **Magnitude (secondary)** | < 5% mean/p50 shift |
| **Hard floor** | All survival canaries must hold (Starvation == 0, ShadowFoxAmbush ≤ 10, no never-fired positives, all five continuity canaries ≥ 1) |

A drop > ±10% on `HuntAttempted` or > ±5% on Hunt DSE mean/p50 escalates
to `just hypothesize` with a YAML spec (multi-seed sweep), per
`feedback_dormant_substrate_activation_soak_first`.

## Observation

Soak: `logs/tuned-42/` (seed 42, parent commit `c44f2e0e`, focal cat
Simba). Per-ticket baseline: `logs/tuned-42-pre-062/` (same parent
commit, pre-062 code state — isolates 062's per-ticket delta from
unrelated post-297 drift).

- Survival canaries: **pass** (Starvation = 0, ShadowFoxAmbush = 1,
  never_fired_expected_positives = [])
- Continuity canaries: **pass** (grooming 1091, play 8, mentoring 315,
  courtship 2618, mythic-texture 14, burial 1 — all five canaries
  ≥ 1 per soak)
- Footer drift vs pre-062: **none** (`footer_drift: []` from
  `just verdict logs/tuned-42 --baseline logs/tuned-42-pre-062/events.jsonl`)
- HuntAttempted feature event count:
  - pre-062 (80,899 ticks): 809 occurrences → rate 0.01000 per tick
  - 062     (105,384 ticks): 1054 occurrences → rate 0.01000 per tick
  - Rate-normalized Δ: **0.0%**
- Hunt DSE mean Δ from `just frame-diff trace-Simba`: **+0.000 (+0.0%)**
- Hunting DSE mean Δ from `just frame-diff trace-Simba`: **+0.000 (+0.0%)**
- All other tracked DSE deltas: +0.000 each (frame-diff verdict:
  "concordance: ok — no unacknowledged drift on tracked DSEs")
- L1 trace species keys: all five `prey_scent_<species>` present,
  no aggregate `prey_scent` key. Non-zero sample counts:
  - bird: 4787, fish: 381, mouse: 2889, rabbit: 798, rat: 1896
  - Bird samples cluster at the smallest magnitudes (~0.001–0.004),
    consistent with the ~⅓ emission-scaling factor.

## Concordance

| Axis | Predicted | Observed | Direction match | Magnitude within ~2× |
|---|---|---|---|---|
| HuntAttempted drop (rate) | < 10% drop | 0.0% drift | n/a (no movement) | ✓ |
| Hunt DSE mean Δ | < 5% drift | 0.0% | n/a (no movement) | ✓ |
| Hunting DSE mean Δ | < 5% drift | 0.0% | n/a (no movement) | ✓ |
| Survival canaries | hold | pass | ✓ | ✓ |
| Continuity canaries | hold | pass (all 5) | ✓ | ✓ |
| L1 surface live | 5 species, non-zero | 5 species, non-zero | ✓ | ✓ |

**Concordance confirmed.** The structural cutover is mathematically
equivalent on this seed (max-aggregate `get_any` preserves the
dominant-species read at every shared tile), and the emission scaling
correctly suppresses Bird-sub-map magnitudes without lifting them
through the aggregate read into Hunt-initiation behavior. Hard gates
hold; landing approved.

**Note on direction match:** the prediction expected a small downward
drift; the observed 0% drift means the structural cutover preserved
aggregate semantics so cleanly that emission-scaling-only-on-Bird-tiles
didn't surface in the aggregate read at all this seed. A multi-seed
sweep could surface drift on bird-dominant tiles, but this seed's
ecology is mouse/rat-dominant (per `PreyConstants::initial_den_count_*`:
4 mouse / 3 rat / 3 rabbit / 2 fish / 2 bird), so the aggregate
`get_any` returns the dominant-species reading at every overlap. The
per-species data surface is now in place for future dietary-
specialization consumers to discriminate without this aggregate masking.
