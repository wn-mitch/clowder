# Ward-placement `cat_value` coefficient — first non-threat-axis lever

**Date:** 2026-05-12
**Ticket:** [298](../open-work/tickets/298-tune-ward-placement-cat-value-coefficient-285296297-architectural-follow-on.md)
**Predecessor evidence:** Four threat-axis (or threat-axis-adjacent) levers ruled out as placement movers across three seeds each — 285 (anchor magnitude), 296 (Logistic curve shape), 297 (third orthogonal threat-axis input), 300 (candidate-step grid). All four produced **byte-identical** `WardPlaced` output on every seed tested. The architectural conclusion documented in `docs/balance/297-fox-patrol-topology-axis.md` iter-2: the threat-axis composition is rank-preserving for the argmax once any threat-side input saturates a sufficient number of tiles, leaving non-threat terms (`+ 0.3 * cat_value`, `- distance_cost`, `jitter`) to determine the argmax.
**Substrate:** `src/systems/coordination.rs:1557` formula `score = unaddressed_threat + W * cat_value - distance_cost + jitter`, where `W = constants.scoring.ward_placement_cat_value_weight` (promoted from hardcoded `0.3` literal in this ticket).

## Hypothesis

Lifting `ward_placement_cat_value_weight` from `0.3` (first-light value from ticket 045) to `0.4` (+33%) increases the cat_value term's share of the residual score budget by ~10% relative to `distance_cost` across typical anchor distances. If the `CatPresenceMap` distinguishes residential cluster tiles from threat-saturated outskirts — the load-bearing premise — the coefficient bump should shift wards toward cluster tiles where foxes are more likely to traverse, lifting `shadow_foxes_avoided_ward_total`. The falsifier is byte-identical placement: if 0.3 → 0.4 holds the argmax steady, every linear term in the current scoring formula is rank-preserving and parameter tuning on this formula cannot move the metric.

**Caveat from ticket 300**: recorded `WardPlaced.location` values are dominated by Path B (cat's current position at self-picked `HerbcraftSetWard`), not by the coordinator's grid-scored target. `ward_placement_cat_value_weight` only affects the directive-driven subset (Path A) of placements, capping expected magnitude below what a pure-scorer regime would predict.

## Methodology — bold-probe (0.4), 285 iteration pattern

Per the user's plan-mode decision: write one yaml per seed (seed-42 primary + seed-99/seed-7 sisters), test the boldest non-baseline value (0.4) first. If 0.4 produces meaningful movement, descend (0.1, 0.2) to map the response curve. If 0.4 fails to move the metric on any seed, escalate the architectural conclusion. Single rep × 900s × release per seed, run through `just hypothesize` (which contains baseline + treatment arms per spec).

**Specs:**
- `docs/balance/hypothesis-298-cat-value-coefficient.yaml` — seed-42 primary.
- `docs/balance/hypothesis-298-cat-value-coefficient-seed99.yaml` — load-bearing (counter has headroom at 20).
- `docs/balance/hypothesis-298-cat-value-coefficient-seed7.yaml` — falsifier (counter at 78).

**Hypothesize output dirs:**
- `logs/sweep-baseline-at-post-297-substrate-dormant-weights-and-the-post-300-candi/` + `-treatment/` (seed-42, auto-slugged)
- `logs/sweep-baseline-298-cat-value-seed99/` + `logs/sweep-298-cat-value-seed99-treatment/`
- `logs/sweep-baseline-298-cat-value-seed7/` + `logs/sweep-298-cat-value-seed7-treatment/`

## Constants landed

**None changed.** `default_ward_placement_cat_value_weight()` ships at `0.3`, preserving byte-identical pre-298 behavior. The promotion lands as a substrate-no-op so the knob is tunable from `SimConstants.scoring` without further code changes; the value defaults to the literal that was previously hardcoded at `coordination.rs:1557`.

```rust
default_ward_placement_cat_value_weight() -> f32     { 0.3 }   // was hardcoded literal 0.3
```

The iter-1 sweep at `W=0.4` tested whether to lift the default; the cross-seed result (below) does not support a lift.

## Observation

### Macro counters — modest seed-7 lift, flat on seeds 42 / 99

| Seed | `shadow_foxes_avoided_ward_total` B → T (Δ) | `wards_placed_total` B → T (Δ) | `deaths_by_cause.ShadowFoxAmbush` B → T (Δ) |
|---|---|---|---|
| **42** | 2 → 2 (**0%**) | 16 → 14 (−2) | 2 → 2 (0) |
| **99** | 20 → 20 (**0%**) | 9 → 9 (0) | 2 → 2 (0) |
| **7** | 78 → 82 (**+5.1%**) | 11 → 13 (+2) | 5 → 5 (0) |

Concordance verdict per hypothesize tool: **wrong-direction on every seed** — the +5.1% on seed-7 falls in the "unchanged" magnitude band of the tool's threshold (the predicted-band `[-20, +30]` for seed-7 is satisfied numerically but the tool labels small-positive deltas as "unchanged" not "increase").

### Spatial topology — placement is NOT byte-identical on 2 of 3 seeds

This is the structurally distinctive finding vs. 285/296/297/300 (which were byte-identical on every seed).

**Seed-42** — Two `WardPlaced` events at `[42, 54]` (Bramble, ticks 1331272 + 1331281) present in baseline, absent in treatment. The remaining 14 placements are at the same seven tiles in both runs with identical multiplicities. The dropped tile (`[42, 54]`) is in the south-east; the metric stays at 2 because that tile didn't intercept any foxes in baseline either — the placement perturbation is at a metric-irrelevant tile.

| Position | Baseline (W=0.3) | Treatment (W=0.4) |
|---|---|---|
| `(33, 10)` | 4 | 4 |
| `(62, 3)` | 2 | 2 |
| `(42, 54)` | **2** | **0** |
| `(42, 36)` | 2 | 2 |
| `(39, 23)` | 2 | 2 |
| `(29, 23)` | 2 | 2 |
| `(38, 22)` | 1 | 1 |
| `(33, 22)` | 1 | 1 |

**Seed-99** — Fully byte-identical placement. Same nine events at the same six unique tiles. The cat_value coefficient bump produces no rank-change on this seed's geometry.

| Position | Baseline (W=0.3) | Treatment (W=0.4) |
|---|---|---|
| `(81, 58)` | 2 | 2 |
| `(77, 45)` | 2 | 2 |
| `(105, 69)` | 2 | 2 |
| `(94, 52)` | 1 | 1 |
| `(93, 52)` | 1 | 1 |
| `(92, 55)` | 1 | 1 |

**Seed-7** — Two new `WardPlaced` events at `[104, 39]` (Wren, ticks 1315924 + 1315933) appear in treatment with no corresponding baseline events. The new tile catches the +4 fox-avoidance lift (78 → 82). Cross-checking the events log, Wren's plan at tick 1315917 is `Witchcraft / SetWard` (not `HerbcraftSetWard`) and her position at tick 1315900 is `[103, 39]`, adjacent to the placement — consistent with Path B (cat's current position at self-picked SetWard) rather than coordinator-directed Path A. The cat_value coefficient still drove the result indirectly: changing the coordinator's earlier placement decisions cascades into different cat positions, which changes which Path-B wards land where.

| Position | Baseline (W=0.3) | Treatment (W=0.4) |
|---|---|---|
| `(92, 48)` | 2 | 2 |
| `(74, 53)` | 2 | 2 |
| `(68, 26)` | 2 | 2 |
| `(65, 37)` | 2 | 2 |
| `(64, 27)` | 2 | 2 |
| `(85, 42)` | 1 | 1 |
| `(104, 39)` | **0** | **2** |

### Continuity canaries — pass on all seeds

| Seed | grooming | courtship | mentoring | play | mythic-texture |
|---|---|---|---|---|---|
| 42 (T) | 1203 | 3582 | 233 | 14 | 43 |
| 99 (T) | 1141 | 4215 | 104 | 5 | 14 |
| 7 (T) | 894 | 1887 | 452 | 11 | 6 |

All five continuity canaries ≥ 1 on every treatment soak. `MatingOccurred` appears in `never_fired_expected_positives` on seed-99 in both baseline and treatment — this is a pre-existing seed-99 demographic state, not a regression caused by W=0.4.

## Concordance

| Artifact | Seed-42 | Seed-99 | Seed-7 |
|---|---|---|---|
| **Hypothesis** | W=0.4 shifts placement toward cluster tiles; metric lifts. | Same. | Same. |
| **Prediction** | `shadow_foxes_avoided_ward_total` Δ ∈ [+50, +300]%. | Δ ∈ [−30, +50]%. | Δ ∈ [−20, +30]%. |
| **Observation** | Δ = 0% (2 → 2). Two wards dropped at metric-irrelevant `[42, 54]`. | Δ = 0% (20 → 20). Placement byte-identical. | Δ = +5.1% (78 → 82). Two new wards at fox-intercept tile `[104, 39]`. |
| **Concordance** | **wrong-direction** (Δ=0% below the +50% floor; placement DID change, but at metric-irrelevant tiles). | "unchanged" (in-band magnitude, no direction signal). | **modest concordance** (in-band magnitude, +5.1% direction signal but below the tool's "increase" threshold). |

### Hard-gate readout (treatment soaks, three seeds)

- `deaths_by_cause.Starvation == 0` → PASS (0 on all three).
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2, 2, 5 respectively).
- `never_fired_expected_positives == 0` → PASS on seeds 42 and 7. On seed-99, `MatingOccurred` never fires in both baseline AND treatment (pre-existing demographic state, not a W=0.4 regression).
- Five continuity canaries each ≥ 1 → PASS on all three.
- Constants-drift-vs-baseline → clean.
- Verdict exit: **pass** on all three.

## Decision

**Keep `W=0.3` default. Land 298 findings-only on the magnitude prediction.**

Three findings drive the decision and the follow-on:

1. **The cat_value coefficient is the first non-byte-identical lever in the 285→298 sequence.** 285 (magnitude), 296 (curve shape), 297 (third orthogonal threat-axis input), and 300 (candidate-step grid) each produced byte-identical placement on every seed tested. 298 produces rank-changing placement on 2 of 3 seeds (seed-42: 2 dropped wards; seed-7: 2 added wards). The threat-axis-rank-preserving conclusion from 297 iter-2 stands; the **non-threat-axis terms are not rank-preserving**.
2. **The rank-change magnitude is too small to justify a default shift.** Seed-7's +5.1% lift on the metric is real (the new wards at `[104, 39]` catch +4 foxes) but falls below the predicted concordance band on the load-bearing seed (seed-42 predicted +50/+300%, observed 0%). Per my own iteration policy in the seed-42 yaml ("Concordant lift on all three seeds → land at 0.4"), the mixed outcome disqualifies the bold-probe value.
3. **The substrate-no-op promotion is the durable win.** Lifting `0.3` from a code literal to `SimConstants.scoring.ward_placement_cat_value_weight` makes the knob tunable for future iterations without source-tree changes. iter-2 can probe descending values (0.1, 0.2) and run multi-rep to validate whether the seed-7 +5.1% is reproducible or single-rep noise.

## Structural-option candidate (per ticket 298 scope step 5)

The ticket scope drafted a structural alternative: **split `cat_value` into `cat_density` (where cats currently live) and `cat_movement_intensity` (where cats traverse).** Inspecting the `CatPresenceMap` populator at `src/systems/disposition.rs::cat_presence_tick` shows the map already gates deposits on `Action::Patrol | Fight | Explore` — it encodes **movement intensity** today, NOT residential density. The structural framing in the ticket scope has the labels reversed: the existing map IS the movement-intensity axis; the missing axis is residential density (depositing on `Action::Sleep | Eat | Groom | Socialize` at a structure or den).

Concrete follow-on shape:
- **Add** a `CatResidenceMap` resource alongside `CatPresenceMap`, depositing on residential-action verbs at structure/den tiles. Decay rate matches `CatPresenceMap` for symmetry.
- **Read** both maps in `compute_ward_placement`, with separate coefficients (`W_movement` and `W_residence`). Default `W_movement = 0.3` (preserves current behavior), `W_residence = 0.0` (dormant first-light).
- **Validate** via a single soak-trace that `W_residence > 0` shifts placement onto den/structure-adjacent tiles, distinct from the patrol-corridor tiles `CatPresenceMap` already favors.

The structural separation prices two distinct ecological intuitions:
- **Movement-intensity wards** (current): protect the corridors cats traverse during patrol/exploration. Existing behavior.
- **Residence wards** (new axis): protect the dens/structures where cats sleep, eat, and recover. Currently absent — wards have no signal for "where cats are vulnerable while resting."

The two intuitions can compose at different coefficients without one dominating the other, addressing the architectural finding from 297 iter-2 that "linear terms in the current scoring formula are rank-preserving" (because there's effectively only one cat-side signal). Splitting the signal into two orthogonal axes is the structural move; tuning one coefficient (this ticket) is the parameter move that the data show is too weak.

## Iteration history

- **iter-1 (2026-05-12):** Promoted `0.3` literal to `SimConstants.scoring.ward_placement_cat_value_weight` (substrate-no-op). Tested W=0.4 across seeds 42/99/7 via single-rep `just hypothesize`. Result: seed-42 drops 2 metric-irrelevant wards (Δ=0%), seed-99 byte-identical (Δ=0%), seed-7 adds 2 metric-relevant wards (Δ=+5.1%). First non-byte-identical lever in 285→298 sequence; magnitude too small to justify a default shift. Land at W=0.3 (unchanged), open structural-option follow-on (`CatResidenceMap` orthogonal axis).
