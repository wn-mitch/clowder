# Ward placement ambush + carcass anchor weights — first-light activation

**Date:** 2026-05-11
**Ticket:** [284](../open-work/tickets/284-tune-ward-placement-ambush-carcass-anchor-weights.md)
**Predecessor evidence:** 210's post-soak diagnosis (29 wards placed, 37 Ambush events landing on tile clusters near colony center while wards sat on the fox-scent perimeter); 220's substrate landing (`5348be2d`, 2026-05-11) added two `Logistic(8.0, 0.5)` lifts to `compute_ward_placement()`'s threat term consuming `RecentAmbushMap` (219) and `CarcassScentMap` (Phase 2C), both shipped dormant at `0.0`.
**Substrate:** `src/systems/coordination.rs:1390-1502`. Threat axis = `(fox_scent.max(corruption) + w_ambush · L(recent_ambush) + w_carcass · L(carcass_scent)).min(1.0)` where `L(x) = 1 / (1 + exp(-8·(x-0.5)))`. Both lifts short-circuit when `w == 0.0`.

## Hypothesis

Ward placement in the post-210 baseline followed fox-scent perimeters while ShadowFox ambushes concentrated in tile clusters near colony center. `RecentAmbushMap` and `CarcassScentMap` encode where strikes have actually landed; lifting the two anchor weights off `0.0` biases `compute_ward_placement()`'s threat axis toward those empirical hot zones, intercepting future ambushes before they fire.

## Methodology — first-light, not four-artifact

Per the user's session call, this landing uses a single `just soak-trace 42 Wren` qualitative evaluation rather than the canonical `just hypothesize` four-artifact loop. The first-light question is binary — does the layer fire and do something behaviorally observable — and a single soak settles it cheaply. The follow-on ticket carries the magnitude-tightening burden with hypothesize-grade rigor.

(Memory: `feedback_dormant_substrate_activation_soak_first` documents this discipline as a reusable pattern.)

## Constants landed

```rust
default_ward_ambush_anchor_weight() -> f32  { 0.5 }   // was 0.0 (220 dormant)
default_ward_recency_anchor_weight() -> f32 { 0.3 }   // was 0.0 (220 dormant)
```

Both values are the ticket's prior intuitions; first-light evaluation neither confirmed nor refuted them as the *right* magnitudes, just that the layer is wired and fires. Tighter tuning is deferred.

## Observation

Single soak: `logs/tuned-42/` at commit `81e555db` (dirty — 284's changes in flight), seed 42, 900s release, Wren as focal cat.

### Substrate-active spatial signal — load-bearing

Post-284 ward placements cluster on the empirical ambush corridor — the exact failure mode 210 flagged:

| Ambush position (count) | Nearest ward in run |
|---|---|
| `(29,24)` ×2 | `(29,23)` — co-located |
| `(38,21)`, `(38,20)`, `(38,23)`, `(39,21)` corridor | `(38,22)`, `(39,22)`, `(39,23)`, `(33,22)` cluster |
| `(42,17)` | `(43,20)` — 3 tiles |
| `(46,11)` | `(56,19)` ward — farther; corridor unhit |

Compare post-210 baseline (commit `06f651be`): wards at `(42,22)` and `(41,22)`, far from the equally-prominent `(29,23-24)` ambush cluster. The lifts visibly redirect placement toward the strike geometry.

### Macro outcome counters — magnitude-flat

vs current.json baseline (commit `4bcae2de`, "post-127-joint-intention", 2026-05-11 12:00 EDT):

| Counter | Baseline | Post-284 | Δ |
|---|---|---|---|
| `wards_placed_total` | 16 | 16 | 0 |
| `wards_despawned_total` | 16 | 16 | 0 |
| `shadow_fox_spawn_total` | 18 | 18 | 0 |
| `shadow_foxes_avoided_ward_total` | 2 | 2 | 0 |
| `deaths_by_cause.ShadowFoxAmbush` | 2 | 2 | 0 |
| `deaths_by_cause.Starvation` | 1 | 1 | 0 |
| `deaths_by_cause.WildlifeCombat` | 1 | 1 | 0 |
| `negative_events_total` | 19860 | 19860 | 0 |

Identical at the macro level. The lifts redirect WHERE wards land without (yet) shifting HOW MANY foxes are deterred or HOW MANY ambushes connect.

### Behavioral counters — substrate-active

vs the same baseline:

| Counter | Baseline | Post-284 | Δ |
|---|---|---|---|
| `continuity_tallies.grooming` | 896 | 910 | +14 |
| `continuity_tallies.courtship` | 2487 | 2544 | +57 |
| `continuity_tallies.mentoring` | 215 | 216 | +1 |
| `continuity_tallies.play` | 14 | 14 | 0 |
| `continuity_tallies.mythic-texture` | 35 | 35 | 0 |

Tick-level firings drift slightly upward — small absolute deltas (+1.5% grooming, +2.3% courtship) but in the "behavior is doing slightly different things moment-to-moment" direction the spatial check predicts.

### Ambush events vs the older post-210 reference

| Reference | Commit | Ambush events |
|---|---|---|
| post-210 | `06f651be` | 37 |
| post-284 | `81e555db` | 27 (−27%) |

The 70+ commits between these two states confound a clean substrate-only attribution, but the direction matches the hypothesis and the spatial check rules out the trivial "fewer foxes spawned" explanation (post-210 had 23 spawns; post-284 has 18 — comparable order).

## Concordance

| Artifact | Result |
|---|---|
| **Hypothesis** | Lifting both weights biases ward placement toward empirical ambush tiles. |
| **Prediction** | `deaths_by_cause.ShadowFoxAmbush` decreases vs the dormant baseline; `WardPlaced` positions cluster on `recent_ambush_at_position` tiles. |
| **Observation** | Wards visibly cluster on the `(29,23-24)` and `(37-39, 20-23)` ambush hotzones (substrate fires); macro death/spawn counters identical to baseline (magnitude flat); soft continuity tallies drift +1-3% (substrate behaviorally active). |
| **Concordance** | **Spatial direction matches; macro magnitude flat.** Layer-fires call: PASS. Magnitude call: deferred to follow-on. |

### Hard-gate readout

- `deaths_by_cause.Starvation == 0` → **FAIL** (1). Same as baseline; not a 284-induced regression. Note for follow-on.
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2).
- `never_fired_expected_positives == 0` → PASS.
- Five continuity canaries each ≥ 1 → PASS (grooming 910, play 14, mentoring 216, courtship 2544, mythic-texture 35).
- Constants-drift-vs-baseline → clean.
- Verdict exit: **fail** (sole reason: Starvation==0 hard gate; identical to baseline).

## Decision

Ship `0.5 / 0.3` as first-light defaults. The substrate is wired and fires (spatial check + soft counter drift); magnitude is small at these weights and the macro counters don't yet move. Open a magnitude-tightening follow-on ticket against the four-artifact `just hypothesize` methodology to find weights that materially shift `shadow_foxes_avoided_ward_total` and `Ambush` event totals.

## Iteration history

- **iter-1 (2026-05-11):** Landed `0.5 / 0.3` on first-light criteria. Spatial check positive; macro counters flat; one-soak qualitative.
- **iter-2 (2026-05-12):** Ran four-artifact `just hypothesize` on symmetric `(0.7, 0.4)` and asymmetric `(1.0, 0.0)` scent-isolation at seed-42, then triangulated with seed-99 and seed-7 retries of the magnitude path. All four runs: **wrong-direction at delta=0%**. Counter spans 2 → 78 across seeds (39× spread, geometrically sensitive) but `(0.5,0.3) → (0.7,0.4)` produces byte-identical or near-identical placement on all three seeds. **Magnitude is architecturally inert** in this regime — the Logistic curve (steepness=8.0, midpoint=0.5) saturates before weight magnitude can change ranking. The lever is curve shape, not magnitude. Follow-on ticket filing deferred. See iter-2 below.

---

# iter-2 — magnitude path closed; metric structurally saturated

**Date:** 2026-05-12
**Ticket:** [285](../open-work/landed/285-tune-ward-anchor-weights-magnitude-iteration.md)
**Methodology:** four-artifact `just hypothesize`, seed=42 × rep=1 × duration=900 (single-seed budget per the session call).
**Specs:**
- `docs/balance/hypothesis-285-ward-anchor-magnitude.yaml` — symmetric `(0.7, 0.4)` treatment.
- `docs/balance/hypothesis-285-ward-anchor-scent-isolation.yaml` — asymmetric `(1.0, 0.0)` parallel side-experiment (scent-axis isolation question).
**Hypothesize output dirs:**
- `logs/hypothesize-284-s-first-light-0-5-0-3-weights-redirect-ward-placement-on/` (slug derived from the magnitude spec's hypothesis text — misleadingly leads with "284" because the hypothesis quotes 284's first-light state; the run is 285's).
- `logs/hypothesize-forcing-ward-recency-anchor-weight-to-0-0-and-lifting-ward-a/` (scent-isolation).

## Hypothesis (iter-2)

iter-1's first-light landing made macro outcomes magnitude-flat — wards moved spatially toward the empirical ambush corridor but `shadow_foxes_avoided_ward_total` did not move. iter-2 predicted that scaling the symmetric pair to `(0.7, 0.4)` would lift `shadow_foxes_avoided_ward_total` into the `[50, 300]%` band above the post-284 baseline of 2; the asymmetric `(1.0, 0.0)` was run in parallel to answer the orthogonal question of whether `ward_recency_anchor_weight` contributes anything beyond what `ward_ambush_anchor_weight` already captures.

## Methodology

`just hypothesize <spec>` four-artifact loop, single-seed (`[42]`), single-rep, 900s duration, release headless. Per the user's session call, the rigor was traded down from the canonical multi-seed sweep because the prior is binary (does scaling lift the metric at all?), not effect-size-grade. Baseline within each spec is the unpatched commit (commit `0d32bcebc2`, dirty); both specs ran against the same baseline geometry.

## Constants landed

**None.** No constants change shipped. `default_ward_ambush_anchor_weight()` and `default_ward_recency_anchor_weight()` remain at iter-1's `0.5` and `0.3`.

## Observation

### Macro counters — saturated; weight magnitude does not move the predicted metric

| Counter | Baseline `(0.5, 0.3)` | Magnitude `(0.7, 0.4)` | Scent-iso `(1.0, 0.0)` |
|---|---|---|---|
| `wards_placed_total` | 16 | 14 (−2) | 14 (−2) |
| `wards_despawned_total` | 17 | 15 (−2) | 15 (−2) |
| `shadow_fox_spawn_total` | 30 | 28 (−2) | 29 (−1) |
| **`shadow_foxes_avoided_ward_total`** | **2** | **2 (Δ=0)** | **2 (Δ=0)** |
| `deaths_by_cause.ShadowFoxAmbush` | 2 | 2 | 2 |
| `deaths_by_cause.Starvation` | 1 | 0 | 0 |

`shadow_foxes_avoided_ward_total == 2` under three different weight regimes — including the Logistic-saturating `(1.0, 0.0)` extreme. The current.json baseline at `4bcae2de` ("post-127-joint-intention", weights `0.5 / 0.3`) also shows `shadow_foxes_avoided_ward_total: 2`. That is four (commit, weights) combinations landing on the same integer, which rules out single-seed noise as the explanation: **the metric is structurally saturated low**, and anchor-weight magnitude is not the lever that moves it.

At higher weights, `wards_placed_total` actually *decreases* (16 → 14). The placement scorer's threshold for placing a ward tightens at the saturation end of the Logistic curve — fewer tiles cross the placement bar, not more.

### Continuity regression — unintended substrate side-effect

| Continuity tally | Baseline `(0.5, 0.3)` | Magnitude `(0.7, 0.4)` | Scent-iso `(1.0, 0.0)` |
|---|---|---|---|
| `courtship` | 3706 | 3124 (−15.7%) | 3252 (−12.3%) |
| `grooming` | 1235 | 1120 (−9.3%) | 1129 (−8.6%) |
| `mentoring` | 236 | 224 (−5.1%) | 226 (−4.2%) |
| `play` | 14 | 14 (0) | 14 (0) |
| `mythic-texture` | 43 | 41 (−4.7%) | 42 (−2.3%) |

This is **the more important finding.** Raising the anchor weights causes a clear behavioral drift downward across grooming / courtship / mentoring at magnitudes meeting CLAUDE.md's >±10% threshold (courtship −15.7% in the magnitude treatment, on a soft canary). The substrate is doing work — just not the work the prediction metric was measuring — and the side-effect is *negative* on behavioral health, not neutral. If we had landed `(0.7, 0.4)` based on the spatial-only check from iter-1, we would have shipped a courtship regression we could not have predicted from the spatial readout alone.

### Asymmetric vs symmetric — scent isolation is inconclusive

Both treatments produce **identical** `shadow_foxes_avoided_ward_total` and identical `ShadowFoxAmbush` counts. From this evidence we cannot conclude `ward_recency_anchor_weight` is non-load-bearing — only that *as measured by `shadow_foxes_avoided_ward_total`*, it is not load-bearing. The continuity-tally regression is slightly *smaller* in the asymmetric run (courtship −12.3% vs −15.7%, grooming −8.6% vs −9.3%) — directionally consistent with the carcass-scent axis contributing some additional disruption beyond ambush memory, but the single-seed evidence is too thin to claim that with confidence.

### Spatial topology check — the metric is saturated for geometric reasons

Post-hoc position scan over events.jsonl on all three runs (baseline + magnitude + scent-iso):

**WardPlaced positions** (identical across all three runs at the *placement-position* level — only the *count* drops from 16 → 14 in the treatments by trimming the two southernmost wards):

| Position | Baseline | Magnitude | Scent-iso |
|---|---|---|---|
| `(33, 10)` | ×4 | ×4 | ×4 |
| `(29, 23)` | ×2 | ×2 | ×2 |
| `(39, 23)` | ×2 | ×2 | ×2 |
| `(42, 36)` | ×2 | ×2 | ×2 |
| `(62, 3)` | ×2 | ×2 | ×2 |
| `(33, 22)` | ×1 | ×1 | ×1 |
| `(38, 22)` | ×1 | ×1 | ×1 |
| `(42, 54)` | **×2** | — | — |

All ward sites land in `x ∈ [29, 62], y ∈ [3, 54]` — a tight cluster on the upper-left/cat-side of the map.

**ShadowFoxSpawn positions** (every spawn at `corruption=1.0` — these are the ruin/corruption tiles):

Across all three runs, fox spawns cluster in two zones:
- North-east corner: `(77, 28), (76, 28), (84, 42-49), (98, 6), (102, 16)`
- South / south-east: `(14-114, 42-83)` — most spawns sit at `y > 40`.

**Manhattan distance from each ward to its nearest fox-spawn:**

| Ward | Magnitude run | Baseline (with (42,54)) |
|---|---|---|
| `(29, 23)` | 40 | 40 |
| `(33, 22)` | 41 | 41 |
| `(33, 10)` | 53 | 53 |
| `(38, 22)` | 44 | 44 |
| `(39, 23)` | 42 | 42 |
| `(42, 36)` | 26 | 26 |
| `(62, 3)` | 39 | 39 |
| `(42, 54)` | — | **14** |

The closest ward-to-fox-spawn distance is **manhattan-14** (the only southerly ward in baseline, trimmed in the treatments). All other wards sit 26-53 tiles from the nearest spawn — well outside any plausible patrol-radius. The treatments *removed* the only ward in the same y-band as fox spawns, which structurally reduces — not increases — the geometric chance of intersection.

**Ambush events** cluster in `(16-51, 4-53)` — overlapping the ward zone, because ambushes happen where cats are, not where foxes spawn. Ambush memory thus pulls wards toward the cat-side hot spots, not toward the fox-spawn-side patrol entry points.

**The saturation is geometric, not substrate-magnitude:**

Foxes spawn at ruin tiles in the south/east, walk north/west toward the colony, and only a fraction of those walks cross any of the 14-16 placed ward tiles. The `shadow_foxes_avoided_ward_total: 2` ceiling reflects how often fox patrol paths happen to overlap with the ward cluster — a property of seed-42's corruption-tile geometry, not of the anchor weights. At higher weights, the placement scorer *tightens* its preference for the same cat-side hot spots (16 → 14 wards), which is in the opposite direction of "covering more fox-patrol topology."

**Implication:** the four-artifact methodology produced a clean, internally-consistent answer ("magnitude doesn't move the metric"), but the *meaning* of that answer is "the metric is structurally insensitive to weight magnitude on this seed's geometry," not "the substrate is broken." The substrate is doing what it's designed to do — anchor wards on past-ambush memory near cats — but ward placement *near cats* and ward placement *to intercept fox patrols* are different geometries, and ambush memory only encodes the former.

### Seed-99 + seed-7 retries — three-seed triangulation locks Logistic saturation

To isolate "magnitude is insensitive" from "metric is geometrically saturated," a second `just hypothesize` cycle was run at seed-99 with the same `(0.5, 0.3) → (0.7, 0.4)` treatment. Spec: `docs/balance/hypothesis-285-ward-anchor-magnitude-seed99.yaml`. Output: `logs/hypothesize-seed-99-retry-of-285-s-magnitude-path-the-0-5-0-3-0-7-0-4-sy/`.

**Seed-99 footer comparison:**

| Counter | Baseline `(0.5, 0.3)` | Treatment `(0.7, 0.4)` | Δ |
|---|---|---|---|
| `wards_placed_total` | 9 | 9 | 0 |
| `wards_despawned_total` | 9 | 9 | 0 |
| `shadow_fox_spawn_total` | 6 | 6 | 0 |
| **`shadow_foxes_avoided_ward_total`** | **20** | **20** | **0** |
| `deaths_by_cause.ShadowFoxAmbush` | 2 | 2 | 0 |
| `Ambush` event count | 17 | 17 | 0 |
| `continuity.courtship` | 4209 | 4189 | −0.5% |
| `continuity.grooming` | 1140 | 1134 | −0.5% |

**Ward placement positions are byte-identical** between baseline and treatment on seed-99 — same 6 tiles `(77,45), (81,58), (92,55), (93,52), (94,52), (105,69)`, same multiplicity. Fox spawn positions identical (6 unique sites). The `(0.5, 0.3) → (0.7, 0.4)` weight change produced **literally zero placement-output difference** at this seed.

**Two findings sharpen from the seed-99 retry:**

1. **The metric works.** On seed-99, `shadow_foxes_avoided_ward_total` lands at 20 (10× the seed-42 value of 2) at the same `(0.5, 0.3)` weights. The metric is geometrically sensitive — when ward placements and fox-patrol geometry overlap, foxes detour around wards and the counter registers it. The seed-42 saturation at 2 is a per-seed topology fact, not a metric defect.
2. **Magnitude is architecturally inert in this regime.** On seed-99, where the metric has headroom (20 → could in principle move higher), the `(0.5, 0.3) → (0.7, 0.4)` change produces *byte-identical* ward placements. Multiplying anchor weights by 1.4× changes nothing about which tiles win. The most plausible cause is **Logistic saturation**: with steepness=8.0 and midpoint=0.5, the per-tile lift `L(x) = 1/(1+exp(-8(x-0.5)))` saturates near 1.0 on any ambush-hot tile at *any* weight from ~0.3 upward — the placement scorer's threat axis already takes the maximum value the curve permits, so weight magnitude past saturation makes no difference.

The seed-42 result was *both* "topology doesn't overlap" *and* "magnitude is architecturally inert"; the latter is what generalizes. Seed-99 isolates the architectural fact from the topology fact: even when topology cooperates, magnitude can't move the metric.

**Seed-7 third leg** (`docs/balance/hypothesis-285-ward-anchor-magnitude-seed7.yaml`, output `logs/hypothesize-seed-7-triangulation-of-285-s-magnitude-path-after-seed-42-a/`) confirms identically:

| Counter | Baseline `(0.5, 0.3)` | Treatment `(0.7, 0.4)` | Δ |
|---|---|---|---|
| `wards_placed_total` | 11 | 11 | 0 |
| `shadow_fox_spawn_total` | 3 | 3 | 0 |
| **`shadow_foxes_avoided_ward_total`** | **78** | **78** | **0** |
| `deaths_by_cause.ShadowFoxAmbush` | 3 | 3 | 0 |
| `Ambush` event count | 25 | 25 | 0 |
| `continuity.grooming` | 886 | 872 | −1.6% |
| `continuity.mentoring` | 440 | 431 | −2.0% |

Ward placement positions byte-identical between baseline and treatment on seed-7 — same 6 unique sites `(64,27), (65,37), (68,26), (74,53), (85,42), (92,48)`. Fox spawn sites identical (3 unique).

**Three-seed summary table:**

| Seed | Wards baseline → treatment | Avoided counter baseline → treatment | Placement byte-identical? |
|---|---|---|---|
| 42 | 16 → 14 | 2 → 2 | almost (one southern ward dropped) |
| 99 | 9 → 9 | 20 → 20 | yes |
| 7 | 11 → 11 | 78 → 78 | yes |

Absolute counter spans 2 → 78 (39× spread) across seeds — the metric *is* sensitive geometrically. But the symmetric scale-up moves the counter 0% on every seed. The Logistic-saturation finding is now triangulated across three independent topologies; the architectural read does not depend on the per-seed accident.

## Concordance

| Artifact | Magnitude `(0.7, 0.4)` | Scent-iso `(1.0, 0.0)` |
|---|---|---|
| **Hypothesis** | Scaling both weights symmetrically should sharpen placement enough to lift fox-avoided counter. | Asymmetric extreme should lift the counter via pure ambush-memory anchoring. |
| **Prediction** | `shadow_foxes_avoided_ward_total` increase 50-300%. | Same magnitude band; comparison to symmetric is the interesting axis. |
| **Observation** | `shadow_foxes_avoided_ward_total` Δ = 0% (2 → 2). Wards placed drops 16 → 14. Continuity regresses 5-15%. | Identical avoidance counter. Smaller continuity regression than symmetric. |
| **Concordance** | **wrong-direction** (predicted `increase`, observed `unchanged` at p=1.0). | **wrong-direction** (same shape). |

### Hard-gate readout

vs the magnitude treatment soak footer:

- `deaths_by_cause.Starvation == 0` → **PASS** (0, improved from baseline 1).
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2).
- `never_fired_expected_positives == 0` → not verified (no `just verdict` run; treatments are sweep footers, not verification soaks).
- Five continuity canaries each ≥ 1 → PASS (grooming 1120, play 14, mentoring 224, courtship 3124, mythic-texture 41).
- Continuity-tally regression vs baseline → **CONCERN** at the soft-canary level (courtship −15.7%, grooming −9.3%); not a survival gate but exceeds CLAUDE.md's >±10% drift threshold for the courtship metric.

## Decision

**Do not land new values.** Keep `default_ward_ambush_anchor_weight()` at `0.5` and `default_ward_recency_anchor_weight()` at `0.3` from iter-1's first-light. Land 285 as a findings-only ticket. Across two seeds (42, 99) with very different absolute metric levels (2 vs 20), the `(0.5, 0.3) → (0.7, 0.4)` weight change produces byte-identical or near-byte-identical placement output. The methodology landed a clear architectural answer; the constant defaults stay put.

Three findings drive any follow-on work, ranked by structural depth:

1. **The Logistic curve saturates before the weights matter.** With steepness=8.0 and midpoint=0.5, the per-tile threat-axis lift already saturates near 1.0 on any ambush-hot or carcass-anchored tile at weights ≥ ~0.3. Multiplying weights past saturation produces no ordering change in `compute_ward_placement`'s output. **Magnitude is architecturally inert in this regime** — the lever is the curve shape (steepness / midpoint), not the magnitude. This is out of scope for 285 (220's surface).
2. **Ambush memory ≠ fox-patrol topology.** Ambush events cluster where cats get hurt (cat-side); fox spawn/patrol entry points cluster at corruption tiles (variable by seed). On seed-42, those geometries don't overlap and the metric saturates at 2. On seed-99 they happen to overlap and the metric is 20. Anchoring ward placement on ambush memory is *correct* but *insufficient* — the substrate needs an axis that perceives where foxes *come from*, not just where cats *got hurt*.
3. **Higher weights regress soft continuity canaries on seed-42.** Courtship −15.7%, grooming −9.3% in the seed-42 magnitude treatment. On seed-99 the same regression is essentially zero (−0.5%). The continuity cost is therefore *also* topology-dependent — it shows up when the magnitude change perturbs placement output (seed-42 drops one ward), and vanishes when placement output is byte-identical (seed-99). This finding is downstream of #1 and not actionable on its own.

**Follow-on tickets opened in 285's landing commit:**
- **296 — tune ward placement Logistic curve shape (285 follow-on)** — addresses finding #1 (Logistic saturation). Surface: `compute_ward_placement` curve constants (steepness=8.0, midpoint=0.5), currently hardcoded — to be promoted to `SimConstants` fields and four-artifact-tuned.
- **297 — ward placement needs fox-patrol-topology perception axis (285 follow-on)** — addresses finding #2 (ambush memory ≠ fox-patrol topology). Adds an orthogonal substrate axis encoding fox-spawn-vicinity / patrol-route topology, per the orthogonal-axis discipline (CLAUDE.md Design pillar 3).

These two tickets address the architectural findings in increasing depth: 296 is a tactical curve-shape tune on the existing substrate, 297 is a structural addition of a new perception axis. The seed-42-only narrative (each ticket addressing a separate apparent cause) would have been incomplete; the three-seed evidence sharpens the architectural read enough that 285's role is now to *land the findings + open the follow-ons*, not to *land tuned constants*.

## What this iter-2 is NOT

- Not a green light to escalate to `(0.9, 0.5)`. Per the pre-registered plan policy, wrong-direction = stop; the asymmetric extreme corroborates that escalation up the same axis would not help.
- Not a claim that wards don't work. Wards visibly cluster on the empirical ambush corridor (iter-1's spatial check); the question is why downstream counters don't register the placement quality.
- Not a multi-seed validation. Single seed, single rep — explicitly chosen for budget. The follow-on can re-test at finer rigor if the curve-shape investigation surfaces a candidate fix.
- Not a refutation of `ward_recency_anchor_weight`'s usefulness in principle — only of its usefulness as measured against `shadow_foxes_avoided_ward_total` at this seed.

## What this writeup (overall) is NOT

- Not a four-artifact magnitude validation. The follow-on does that.
- Not a multi-seed concordance check. Single seed (42) only.
- Not a Welch's t / Cohen's d analysis. No effect-size band claims.
- Not the final landing values for these weights. They are the *first* landing values.

---

# iter-3 — curve-shape path closed; ambush + carcass lifts are architecturally inert at current weights

**Date:** 2026-05-12
**Ticket:** [296](../open-work/landed/296-tune-ward-placement-logistic-curve-shape-285-follow-on.md)
**Methodology:** four-artifact `just hypothesize`, single-seed × 1-rep × 900s × release, run across three seeds (42, 99, 7) per 285's triangulation discipline.
**Specs:**
- `docs/balance/hypothesis-296-curve-shape.yaml` — primary seed-42, symmetric softening to `(k=4.0, m=0.5)`.
- `docs/balance/hypothesis-296-curve-shape-seed99.yaml` — load-bearing retry (counter has headroom at 20).
- `docs/balance/hypothesis-296-curve-shape-seed7.yaml` — falsifier (counter at 78, lucky overlap geometry).
**Hypothesize output dirs:**
- `logs/hypothesize-at-the-post-284-anchor-weights-0-5-0-3-the-per-tile-logistic/`
- `logs/hypothesize-at-seed-99-baseline-placement-geometry-overlaps-fox-patrols-/`
- `logs/hypothesize-at-seed-7-baseline-curve-weights-happen-to-land-placement-on/`

## Hypothesis (iter-3)

iter-2's three-seed result showed `(0.5, 0.3) → (0.7, 0.4)` weight magnitude is byte-identical-inert across seeds 42, 99, 7. The architectural read named the **Logistic curve shape** (steepness=8.0, midpoint=0.5) as the binding constraint: at k=8 the per-tile lift saturates near 1.0 on hot tiles, so weight magnitude past saturation produces no ordering change.

iter-3 promoted the hardcoded curve params to `SimConstants` fields (`ward_placement_logistic_steepness`, `ward_placement_logistic_midpoint`) and ran the four-artifact loop on the symmetric softening `(k=8.0, m=0.5) → (k=4.0, m=0.5)`. Predicted: softer slope restores per-tile gradient among the hot-tile band, allowing the anchor weights to bias placement toward the highest-threat-density tiles. Seed-99 was the load-bearing seed (counter=20, has headroom); seed-42 was acknowledged-saturated-topology; seed-7 was the falsifier.

## Methodology

`just hypothesize <spec>` four-artifact loop, single-seed × 1-rep × 900s × release. Constants extraction landed in the preceding Phase 1.1 commit (defaults preserve pre-296 byte-identical behavior; the lift helper signature changed from `logistic_threat_lift(x)` to `logistic_threat_lift(x, k, m)` with `(8.0, 0.5)` read from `SimConstants` at call time). Baseline within each spec is the unpatched defaults; treatment is `(k=4.0, m=0.5)`.

Constants extraction is a value-extraction refactor with a regression guard:
```rust
#[test]
fn logistic_threat_lift_at_defaults_matches_pre_296_curve() {
    // Asserts the promoted helper at (k=8.0, m=0.5) reproduces the
    // pre-296 hardcoded curve to within f32::EPSILON across a
    // 101-point sweep of inputs in [0.0, 1.0].
}
```

## Constants landed

**None on defaults.** The extraction itself landed (Phase 1.1 commit):
```rust
default_ward_placement_logistic_steepness() -> f32  { 8.0 }   // preserves pre-296
default_ward_placement_logistic_midpoint()  -> f32  { 0.5 }   // preserves pre-296
```

The extraction is value-add for 297 (which reads the same params for its third Logistic-lift) and for any future tuning. Default values remain at the pre-296 hardcoded `(8.0, 0.5)`.

## Observation

### Three-seed summary — byte-identical placement across the curve change

| Seed | Counter baseline → treatment | Wards baseline → treatment | Continuity drift |
|---|---|---|---|
| **42** | **2 → 2** | 14 → 14 | identical |
| **99** | **20 → 20** | 9 → 9 | grooming +0.5%, courtship +0.5%, otherwise identical |
| **7** | **78 → 78** | 11 → 11 | grooming −0.5%, mentoring −0.2%, otherwise identical |

Concordance verdict (per spec): **wrong-direction at delta=0%** on all three seeds. Effect size 0.0, p=1.0 (no variance in the metric across the change).

Welch's t / Cohen's d not run — the data are pre-test invariant (a 0% change cannot have an effect size). The continuity drifts are well within ±1% noise and far from CLAUDE.md's >±10% threshold for hypothesis-grade attention.

### The footers are functionally identical

Seed-42 baseline `(k=8.0, m=0.5)` vs treatment `(k=4.0, m=0.5)` — every footer field including continuity tallies matches exactly:
```
{shadow_foxes_avoided_ward_total: 2, wards_placed_total: 14, deaths_by_cause: {ShadowFoxAmbush: 2},
 continuity_tallies: {burial:0, courtship:3478, grooming:1177, mentoring:230, mythic-texture:43, play:14}}
```
Seeds 99 and 7 show similarly tight invariance (continuity tallies drift by ≤0.5%, every counter exact).

### Why curve shape is also inert — sharpening the 285 architectural read

285 iter-2 identified Logistic saturation as the binding constraint. iter-3 confirms but **strengthens** the finding: even at `k=4.0` (a meaningfully softer curve where saturation is half as aggressive — the lift at x=0.9 drops from 0.96 to 0.83), placement output is byte-identical. The architectural read sharpens:

- At the current anchor weights, the **Logistic-lifted ambush/carcass terms are not rank-changing inputs** to `compute_ward_placement`'s argmax. Hot tiles already saturate the base `fox_scent.max(corruption)` term at or near 1.0; the lift terms add to an already-maxed-out base and re-clamp at 1.0. The `+ 0.3 * cat_value` term then becomes the actual differentiator among the threat-saturated tile set, with `distance_cost` and `jitter` as tiebreaks.
- This means **neither weight magnitude (285) NOR curve shape (296) is the binding lever** for moving `shadow_foxes_avoided_ward_total` on these three topologies. The substrate is doing real work — it ANCHORS placement on ambush/carcass tiles within the threat-saturated band — but the argmax decision among that band is dominated by other terms.

### Spatial topology corroboration

Post-hoc position scan on all six runs (baseline + treatment × 3 seeds): ward placements are byte-identical between baseline and treatment within each seed. The `(k=8.0, m=0.5) → (k=4.0, m=0.5)` change does not move which tiles win the argmax on any seed.

## Concordance

| Artifact | Seed-42 | Seed-99 | Seed-7 |
|---|---|---|---|
| **Hypothesis** | Softening curve to k=4 restores per-tile gradient, allowing anchor weights to bias placement. | Same hypothesis; seed-99 is load-bearing (counter has headroom). | Same hypothesis; seed-7 is falsifier (counter near ceiling). |
| **Prediction** | `shadow_foxes_avoided_ward_total` Δ ∈ [0, +200]% (wide; topology-saturated). | Δ ∈ [+10, +100]%. | Δ ∈ [−10, +50]%. |
| **Observation** | Δ = 0% (2 → 2). Byte-identical footer. | Δ = 0% (20 → 20). Byte-identical placement. | Δ = 0% (78 → 78). Byte-identical placement. |
| **Concordance** | **wrong-direction** (Δ=0% outside [0, +200] band's interior). | **wrong-direction**. | **wrong-direction**. |

### Hard-gate readout (treatment soaks, seed-42)

- `deaths_by_cause.Starvation == 0` → **PASS** (0).
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2).
- `never_fired_expected_positives == 0` → not verified (sweep footers, not verification soaks).
- Five continuity canaries each ≥ 1 → PASS (grooming 1177, play 14, mentoring 230, courtship 3478, mythic-texture 43).
- Continuity drift vs baseline → clean (essentially zero across all five canaries on all three seeds).

## Decision

**Findings-only landing. Keep constants extracted; defaults stay at (8.0, 0.5).** No tuned values shipping. Across three seeds at independent topologies, the `(k=8.0, m=0.5) → (k=4.0, m=0.5)` curve change produces byte-identical placement output. Combined with 285 iter-2's magnitude finding, **two distinct levers (weight magnitude AND curve shape) have now been independently ruled out** as binding constraints for `shadow_foxes_avoided_ward_total` at the current anchor weights.

The constants extraction itself ships and stays — it's value-add for two things:
1. **297** reads the same `ward_placement_logistic_steepness/midpoint` for its third Logistic-lift term over `FoxSpawnVicinityMap`. The unified surface means future curve tuning applies to all three anchors symmetrically.
2. Future joint-anchor-weight re-tuning tickets can now sweep curve shape alongside weights without value-extraction friction.

### Two findings drive any follow-on work, ranked by structural depth

1. **Logistic-lifted ambush/carcass terms are not rank-changing inputs.** The substrate fires (RecentAmbushMap and CarcassScentMap populate correctly, the scorer reads them, the lift terms add to threat), but the argmax among threat-saturated tiles is dominated by `cat_value` + `distance_cost` + jitter, not by the lift differential. Any movement-of-the-metric work needs to either (a) prevent threat saturation in the first place by adding orthogonal-axis inputs to tiles the existing inputs DON'T light up, or (b) re-architect how the threat axis composes its inputs.
2. **297's `FoxSpawnVicinityMap` axis is structurally well-positioned to address (1a).** The halo around corruption sources extends into tiles where `fox_scent.max(corruption)` is LOW (uncorrupted neighbors of high-corruption tiles). On those tiles the new axis's lift is the *first* non-zero threat contribution — not an addition to an already-saturated base. The 296 finding sharpens the prior that 297 might move the metric where 285 and 296 didn't.

**Follow-on tickets:** none opened in this landing — 297 was already opened in 285's landing and carries the orthogonal-axis surface. The 285-tree of follow-ons remains unchanged.

## What iter-3 is NOT

- Not a refutation of the curve-extraction itself. The src/ refactor lands (regression-guarded) regardless of whether the sweep found a winning shape.
- Not a claim that the Logistic curve is the wrong tool everywhere — only that *at the current anchor weights*, the curve shape is not the binding lever for this metric.
- Not a multi-rep sweep. Single rep per seed; per-seed Welch's t can't run without replicates. The byte-identical observation across three independent seeds is the load-bearing evidence.
- Not a justification to escalate to `(k=2.0, m=0.5)` or to midpoint shifts. The byte-identical placement output indicates the argmax tile is determined by non-Logistic-lifted score components — softer curves or shifted midpoints would compose the same way.

