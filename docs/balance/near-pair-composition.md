# Near-pair composition — ticket 506 (480 child)

## Iteration 1 — restrict NearPairCache to cats + wildlife (2026-07-05)

### Hypothesis
`NearPairCache` admitting prey and items (~26k of ~26k pairs non-cat at
seed 42) multiplies the social pipeline's per-tick work ~×800 and
inflates every cat's social graph with meaningless prey familiarity
(cat×prey `Relationships` entries feed `social_weight` /
`bond_asymmetry` sums). Restricting admission to
`Or<(With<CatBeliefs>, With<WildAnimal>)>` deletes phantom substrate:
large perf win, modest and explainable social-metric drift, no
survival-shape change.

### Predictions (pre-registered before the observation soak)
| # | Prediction | Band |
|---|---|---|
| P1 | `ticks_per_sec` vs the `post-459-490-instrumentation` baseline (67.7) | **+25–60%** (the 505-stack run sat at +13%; 506 removes update_near_pair_cache ~7% incl, passive_familiarity ~6.8% self, integrate_beliefs ballast ~14% self, copresence/iter_for residuals) |
| P2 | Survival gates | `Starvation == 0`, `ShadowFoxAmbush <= 10` hard |
| P3 | Continuity canaries | grooming / play / mentoring / courtship each ≥ 1 |
| P4 | Cache size | tens of pairs, not ~26k (structural — unit test + this soak's flamegraph shows update_near_pair_cache + passive_familiarity + integrate_beliefs all shrunk) |
| P5 | Social-family drift | coordinator/social DSE scores shift (deflated familiarity sums); frame-diff vs `logs/tuned-42-837b1aaf/trace-Simba.jsonl` shows Social/Groom/Patrol-family |Δ| > Hunt/Forage-family |Δ|; Hunt/Forage scoring shape stable |
| P6 | Reproduction | `kittens_born >= 1`; `MatingOccurred` fires |

### Observation
Run `logs/tuned-42-3e4f7caf` (900s soak-trace 42/Simba at the 506
commit), verdict **concern** (drift-band, not gate):
- P1: ticks_per_sec **112.0 (+65.4%)** vs baseline 67.7.
- P2: Starvation 0, deaths_by_cause **empty** (zero deaths of any
  kind), ShadowFoxAmbush deaths 0.
- P3: grooming 3047 / play 32 / mentoring 981 / courtship 14555 — all
  ≥ 1 (courtship per-tick rate ≈ flat once normalized for +65% ticks).
- P4: composition unit test green; post-506 flamegraph
  (`42-3e4f7caf356f`): update_near_pair_cache, passive_familiarity,
  track_sustained_copresence, integrate_beliefs **all below the
  report threshold** (were 7.0/6.8/19.7/14.1). New #1 is
  prey_ai/try_detect_cat 24.7% incl — the pre-existing frame plan.md
  risk #2 already assigns to 266's alert-set gating.
- P5: frame-diff (cross-commit advisory) top movers are the
  social/self-care family — groom_self +31%, sleep +36%,
  groom_other +24%, caretake +19% — consistent with un-diluted
  social scalars. hunt mean **-22%** (larger than the "stable shape"
  predicted); forage +16%, nourishment +18%, so food intake shifted
  channel rather than degrading.
- P6: kittens_born **4** (baseline 2); MatingOccurred fired.
- Unpredicted drift: shadow_foxes_avoided_ward_total 22 → 212 per
  10kt, ward_siege_started 1.8 → 16.2 per 10kt; fulfillment -38%,
  happiness -13%; one founder-dispersion window below floor (7.1
  tiles at elapsed 18000).

### Concordance
P1 concordant (above band top: +65.4 vs +25–60 — under-predicted).
P2/P3/P4/P6 concordant. P5 partially concordant: the social-family
dominance held, but hunt's -22% mean exceeds "shape stable".

**Mechanism for the unpredicted drift** (dilution removal, the
silent-zero family in its averaging variant): pre-506,
`social_status_distress`'s bond_asymmetry arm averaged fondness over
`iter_for(nearest_other)` — hundreds of prey entries at fondness 0
pinned the average ≈ 0, silently zeroing the scalar. Post-506 the
average runs over real cat bonds; social distress, comfort-seeking
(groom/sleep lifts), and tighter clustering (dispersion dip,
cuddle-puddle signature) all activate. Tighter clustering pulls
shadowfox approaches into ward radii → avoidance/siege counts jump
while **zero cats die** — the defense surface is working, not
failing. Welfare axes (fulfillment/happiness) are now measured
against constants tuned during the diluted era — recalibration
spun out to ticket 507 rather than smuggled in here.
